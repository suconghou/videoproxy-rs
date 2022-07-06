use std::{
    collections::HashMap,
    ops::{AddAssign, SubAssign},
    sync::Arc,
    time::Duration,
};

use actix_web::web::{self, Bytes};
use awc::Client;
use tokio::sync::{Mutex, RwLock};

use crate::{cache::map::CACHEDATA, request};

lazy_static! {
    static ref THREAD: Mutex<i32> = Mutex::new(0);
    static ref PROCESS: RwLock<HashMap<String, bool>> = RwLock::new(HashMap::new());
}

const MAX_THREAD: i32 = 5;

pub async fn put_task(client: web::Data<Client>, uid: String, url: String) -> Option<Arc<Bytes>> {
    let limit = 15 << 20;
    let ttl = 120;
    let real = || async {
        let t = Duration::from_millis(200);
        loop {
            {
                let mut n = THREAD.lock().await; // 持有锁，保持判断和自增原子性
                if n.lt(&MAX_THREAD) {
                    n.add_assign(1);
                    break;
                }
                // 离开作用域时释放锁
            }
            {
                if PROCESS.read().await.contains_key(&uid) {
                    THREAD.lock().await.add_assign(1);
                    break;
                }
            }
            tokio::time::sleep(t).await;
        }
        let r = request::req_get(&client, &url, limit).await;
        THREAD.lock().await.sub_assign(1); // 直到return时才释放锁
        r.ok()
    };
    CACHEDATA.load_or_store(&uid, real, ttl).await
}

pub async fn get_task(uid: &String) -> Option<Arc<Bytes>> {
    let real = || async { None };
    PROCESS.write().await.insert(uid.to_owned(), true);
    let item = CACHEDATA.load_or_store(uid, real, 3).await;
    PROCESS.write().await.remove(uid);
    item
}

pub async fn thread() -> i32 {
    THREAD.lock().await.clone()
}
