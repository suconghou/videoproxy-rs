use std::{
    sync::{atomic, Arc},
    time::Duration,
};

use actix_web::web::{self, Bytes};
use awc::Client;

use crate::{cache::map::CACHEDATA, request};

static THREAD: atomic::AtomicU32 = atomic::AtomicU32::new(0);
const MAX_THREAD: u32 = 5;

pub async fn put_task(client: web::Data<Client>, uid: String, url: String) -> Option<Arc<Bytes>> {
    let limit = 15 << 20;
    let ttl = 120;
    let real = || async {
        let t = Duration::from_millis(200);
        loop {
            if THREAD.load(atomic::Ordering::Relaxed) >= MAX_THREAD {
                tokio::time::sleep(t).await;
            }
            break;
        }
        THREAD.fetch_add(1, atomic::Ordering::Relaxed);
        let r = request::req_get(&client, &url, limit).await;
        THREAD.fetch_sub(1, atomic::Ordering::Relaxed);
        if let Ok(res) = r {
            return Some(res);
        }
        return None;
    };
    CACHEDATA.load_or_store(&uid, real, ttl).await
}

pub async fn get_task(uid: &String) -> Option<Arc<Bytes>> {
    let real = || async { None };
    let item = CACHEDATA.load_or_store(uid, real, 1).await;
    item
}

pub fn thread() -> u32 {
    THREAD.load(atomic::Ordering::Relaxed)
}
