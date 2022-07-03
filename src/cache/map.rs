use actix_web::web::Bytes;
use serde_json::value::Value;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{collections::HashMap, sync::Mutex};
use tokio::sync::watch;

// HashMap<String, Value> 为我们缓存的JSON对象
lazy_static! {
    pub static ref CACHEJSON: CacheMap<HashMap<String, Value>> = CacheMap::new();
    pub static ref CACHEDATA: CacheMap<Bytes> = CacheMap::new();
}

pub struct CacheMap<V> {
    data: Mutex<HashMap<String, TaskItem<V>>>,
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
            data: Mutex::new(HashMap::new()),
        }
    }

    pub fn expire(&self) {
        let mut pending = self.data.lock().unwrap();
        pending.retain(|_, v| v.t.elapsed() < v.ttl || v.rx.is_some());
    }

    pub fn len(&self) -> usize {
        return self.data.lock().unwrap().len();
    }

    pub async fn load_or_store<F, Fut>(&self, key: &String, f: F, ttl: u64) -> Option<Arc<V>>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Option<Arc<V>>>,
    {
        self.expire();
        match {
            let mut pending = self.data.lock().unwrap();
            match pending.get(key) {
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
                        ttl: Duration::from_secs(ttl),
                    };
                    pending.insert(key.clone(), item);
                    NewlyPending(tx)
                }
            }
        } {
            AlreadyPending(mut rx) => {
                let cache_it = |data: Option<Arc<V>>| -> Option<Arc<V>> {
                    if data.is_some() {
                        let mut pending = self.data.lock().unwrap();
                        if let Some(mut o) = pending.get_mut(key) {
                            o.data = data.clone();
                            o.rx = None;
                            o.t = Instant::now();
                        } else {
                            pending.insert(
                                key.clone(),
                                TaskItem {
                                    data: data.clone(),
                                    rx: None,
                                    t: Instant::now(),
                                    ttl: Duration::from_secs(ttl),
                                },
                            );
                        }
                    }
                    data
                };
                if rx.changed().await.is_ok() {
                    match rx.borrow().clone() {
                        Some(v) => cache_it(Some(v)),
                        None => cache_it(f().await),
                    }
                } else {
                    cache_it(f().await)
                }
            }
            NewlyPending(tx) => {
                let v = f().await;
                self.data.lock().unwrap().insert(
                    key.clone(),
                    TaskItem {
                        data: v.clone(),
                        rx: None,
                        t: Instant::now(),
                        ttl: Duration::from_secs(ttl),
                    },
                );
                tx.send(v.clone()).unwrap_or_default();
                v
            }
        }
    }
}
