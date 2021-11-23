use crate::cache::map::CACHE;
use actix_web::client::Client;
use actix_web::http::StatusCode;
use actix_web::web;
use core::time::Duration;
use serde_json::value::Value;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::Arc;

const LIMIT: usize = 1024 * 1024 * 5;

pub async fn getplayer(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<Arc<HashMap<String, Value>>, Box<dyn Error>> {
    let real = || async {
        if let Ok(res) = getnetplayer(client, vid).await {
            return Some(res);
        }
        return None;
    };
    let item = CACHE.load_or_store(vid, real).await;
    if let Some(res) = item {
        return Ok(res);
    }
    getnetplayer(client, vid).await
}

async fn getnetplayer(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<Arc<HashMap<String, Value>>, Box<dyn Error>> {
    let video_url = "https://youtubei.googleapis.com/youtubei/v1/player?key=AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
    let req = serde_json::json!({
        "videoId": vid,
        "context": {
            "client": {
                "clientName": "Android",
                "clientVersion": "16.13.35"
            }
        }
    });

    let mut response = client
        .post(video_url)
        .timeout(Duration::from_secs(10))
        .set_header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/94.0.4606.81 Safari/537.36")
        .set_header("Content-Type", "application/json")
        .set_header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .send_json(&req)
        .await?;
    match response.status() {
        StatusCode::OK => {
            let res = response
                .json::<HashMap<String, Value>>()
                .limit(LIMIT)
                .await?;
            Ok(Arc::new(res))
        }
        _ => {
            println!("status: failed {} {}", vid, response.status());
            let res = response.body().limit(LIMIT).await?;
            println!("{:?}", res);
            Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("{} {}", vid, response.status()),
            )))
        }
    }
}
