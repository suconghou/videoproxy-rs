use serde_json::value::Value;
use std::future::Future;
use std::time::{Duration, Instant};
use std::{collections::HashMap, sync::Mutex};
use tokio::sync::watch;

lazy_static! {
    pub static ref CACHE: CacheMap<HashMap<String, Value>> = CacheMap::new(3600);
}

pub struct CacheMap<V: Clone> {
    data: Mutex<HashMap<String, TaskItem<V>>>,
    ttl: Duration,
}

struct TaskItem<V: Clone> {
    data: Option<V>,
    rx: Option<watch::Receiver<Option<V>>>,
    t: Instant,
}

type PendingReceiver<V> = watch::Receiver<Option<V>>;
type PendingSender<V> = watch::Sender<Option<V>>;

enum GetPending<V> {
    AlreadyPending(PendingReceiver<V>),
    NewlyPending(PendingSender<V>),
}

use GetPending::*;

impl<V: Clone> CacheMap<V> {
    pub fn new(ttl: u64) -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(ttl),
        }
    }

    fn expire(&self) {
        let mut pending = self.data.lock().unwrap();
        pending.retain(|_, v| v.t.elapsed() < self.ttl);
    }

    pub async fn load_or_store<F, Fut>(&self, key: &String, f: F) -> Option<V>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Option<V>>,
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
                    };
                    pending.insert(key.clone(), item);
                    NewlyPending(tx)
                }
            }
        } {
            AlreadyPending(mut rx) => {
                let cache_it = |data: Option<V>| -> Option<V> {
                    if data.is_some() {
                        let mut pending = self.data.lock().unwrap();
                        pending.insert(
                            key.clone(),
                            TaskItem {
                                data: data.clone(),
                                rx: None,
                                t: Instant::now(),
                            },
                        );
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
                tx.send(v.clone()).unwrap_or_default();
                v
            }
        }
    }
}
