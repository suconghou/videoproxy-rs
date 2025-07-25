use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use actix_web::web::{self, Bytes};
use awc::Client;
use tokio::sync::{RwLock, Semaphore, SemaphorePermit};

use crate::{cache::map::CACHEDATA, request};
const MAX_THREAD: usize = 5;

static THREAD: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(MAX_THREAD));
static PROCESS: LazyLock<RwLock<HashMap<String, bool>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn put_task(client: web::Data<Client>, uid: String, url: String) -> Option<Arc<Bytes>> {
    let limit = 15 << 20;
    let ttl = 120;
    let real = || async {
        // 检查当前任务是否是高优先级
        let is_priority = PROCESS.read().await.contains_key(&uid);
        // 如果不是高优先级任务，则必须获取一个信号量许可
        // 如果是高优先级任务，则 _permit 为 None，直接执行
        let _permit: Option<SemaphorePermit> = if !is_priority {
            match THREAD.acquire().await {
                // .acquire() 会等待，直到有可用的许可
                Ok(p) => Some(p),
                Err(_) => return None, // 如果信号量已关闭，则无法继续
            }
        } else {
            None
        };
        let result = request::req_get(&client, &url, limit).await;
        result.ok() // 当这个闭包结束时，_permit (如果存在) 会被自动 drop， 从而释放信号量许可，让其他等待的任务可以继续。
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

pub fn thread() -> usize {
    THREAD.available_permits()
}
