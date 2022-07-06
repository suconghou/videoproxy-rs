use actix_web::web::Bytes;
use serde_json::value::Value;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{watch, RwLock, RwLockWriteGuard};

// HashMap<String, Value> 为我们缓存的JSON对象
lazy_static! {
    pub static ref CACHEJSON: CacheMap<HashMap<String, Value>> = CacheMap::new();
    pub static ref CACHEDATA: CacheMap<Bytes> = CacheMap::new();
}

pub struct CacheMap<V> {
    data: RwLock<HashMap<String, TaskItem<V>>>,
}

struct TaskItem<V> {
    data: Option<Arc<V>>,
    rx: Option<watch::Receiver<Option<Arc<V>>>>,
    t: Instant,
    ttl: Duration,
}

type PendingReceiver<V> = watch::Receiver<Option<V>>;
type PendingSender<V> = watch::Sender<Option<V>>;

enum GetPending<V> {
    AlreadyPending(PendingReceiver<V>),
    NewlyPending(PendingSender<V>),
}

use GetPending::*;

impl<V> CacheMap<V> {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }

    pub async fn expire(&self) {
        let mut pending = self.data.write().await;
        pending.retain(|_, v| v.t.elapsed() < v.ttl);
    }

    pub async fn len(&self) -> usize {
        self.data.read().await.len()
    }

    pub async fn load_or_store<F, Fut>(&self, key: &String, f: F, ttl: u64) -> Option<Arc<V>>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Option<Arc<V>>>,
    {
        self.expire().await;
        match {
            let mut data = self.data.write().await;
            match data.get(key) {
                Some(item) => match &item.data {
                    Some(v) => return Some(v.clone()),
                    None => AlreadyPending(item.rx.as_ref().unwrap().clone()),
                },
                None => {
                    let (tx, rx) = watch::channel(None);
                    let item = TaskItem {
                        data: None,
                        rx: Some(rx),
                        t: Instant::now(),
                        ttl: Duration::from_secs(ttl * 60), // 新插入的任务有60倍ttl的执行时间，即如果我们有任务要缓存5s,则有5分钟的执行时间,5分钟内此任务不会清理，任务执行完毕后又有5s缓存时间
                    };
                    data.insert(key.clone(), item);
                    NewlyPending(tx)
                }
            }
        } {
            AlreadyPending(mut rx) => {
                let cache_it =
                    |v: Arc<V>,
                     mut data: RwLockWriteGuard<HashMap<String, TaskItem<V>, RandomState>>|
                     -> Option<Arc<V>> {
                        if let Some(mut o) = data.get_mut(key) {
                            o.data = Some(v.clone());
                            o.rx = None;
                            o.t = Instant::now();
                        } else {
                            data.insert(
                                key.clone(),
                                TaskItem {
                                    data: Some(v.clone()),
                                    rx: None,
                                    t: Instant::now(),
                                    ttl: Duration::from_secs(ttl),
                                },
                            );
                        }
                        Some(v.clone())
                    };
                match rx.changed().await {
                    Ok(_) => match rx.borrow().clone() {
                        Some(v) => cache_it(v, self.data.write().await),
                        None => match f().await {
                            Some(v) => cache_it(v, self.data.write().await),
                            None => None,
                        },
                    },
                    Err(_) => match f().await {
                        Some(v) => cache_it(v, self.data.write().await),
                        None => None,
                    },
                }
            }
            NewlyPending(tx) => {
                let res = f().await;
                // 我们必须保证data和rx不能都是None
                // 但是无论怎样，我们都必须重新设置ttl,取消之前的60倍ttl
                {
                    let mut data = self.data.write().await;
                    match data.get_mut(key) {
                        Some(mut o) => {
                            // HashMap中存在，则我们需要更新他
                            if res.is_some() {
                                o.data = res.clone();
                                o.rx = None;
                                o.t = Instant::now();
                                o.ttl = Duration::from_secs(ttl);
                            } else {
                                // 本次没有获取到数据，我们仅修改ttl,无需修改t
                                o.ttl = Duration::from_secs(ttl);
                            }
                        }
                        None => {
                            // 不存在，如果我们本次获取到了数据，则插入
                            if res.is_some() {
                                data.insert(
                                    key.clone(),
                                    TaskItem {
                                        data: res.clone(),
                                        rx: None,
                                        t: Instant::now(),
                                        ttl: Duration::from_secs(ttl),
                                    },
                                );
                            }
                            // else 本次也没获取到数据，我们不需要插入
                        }
                    };
                }
                tx.send(res.clone()).unwrap_or_default();
                res
            }
        }
    }
}
