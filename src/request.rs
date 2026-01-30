use crate::cache::map::{CACHEDATA, CACHEJSON};
use actix_web::http::StatusCode;
use actix_web::http::header::{ACCEPT_LANGUAGE, HeaderName, USER_AGENT};
use actix_web::web::{self, Bytes};
use awc::Client;
use core::time::Duration;
use serde_json::value::Value;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::Arc;

const UA: (HeaderName, &str) = (
    USER_AGENT,
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.0.0 Safari/537.36",
);
const AL: (HeaderName, &str) = (ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8");
const TIMEOUT: std::time::Duration = Duration::from_secs(10);

pub async fn getplayer_cache(
    client: &web::Data<Client>,
    vid: &String,
    ttl: u64,
) -> Result<Arc<HashMap<String, Value>>, Box<dyn Error>> {
    let limit = 5 << 20;
    let real = || async { getplayer(client, vid, limit).await.ok() };
    let item = CACHEJSON.load_or_store(vid, real, ttl).await;
    if let Some(res) = item {
        return Ok(res);
    }
    getplayer(client, vid, limit).await
}

async fn getplayer(
    client: &web::Data<Client>,
    vid: &String,
    limit: usize,
) -> Result<Arc<HashMap<String, Value>>, Box<dyn Error>> {
    let video_url = "https://www.youtube.com/youtubei/v1/player?key=AIzaSyB-63vPrdThhKuerbB2N_l7Kwwcxj6yUAc&prettyPrint=false";
    let req = serde_json::json!({
        "videoId": vid,
        "context": {
            "client": {
                "clientName": "IOS",
                "clientVersion": "19.45.4",
                "deviceModel": "iPhone16,2",
            }
        }
    });

    let mut response = client
        .post(video_url)
        .timeout(TIMEOUT)
        .content_type("application/json")
        .insert_header((
            USER_AGENT,
            "com.google.ios.youtube/19.45.4 (iPhone16,2; U; CPU iOS 18_1_0 like Mac OS X;)",
        ))
        .insert_header(AL)
        .send_json(&req)
        .await?;
    match response.status() {
        StatusCode::OK => {
            let res = response
                .json::<HashMap<String, Value>>()
                .limit(limit)
                .await?;
            Ok(Arc::new(res))
        }
        _ => {
            println!("status: failed {} {}", vid, response.status());
            let res = response.body().limit(limit).await?;
            println!("{:?}", res);
            Err(Box::new(io::Error::other(format!(
                "{} {}",
                vid,
                response.status()
            ))))
        }
    }
}

pub async fn req_get_cache(
    client: &web::Data<Client>,
    url: &String,
    ttl: u64,
    limit: u32,
) -> Result<Arc<Bytes>, Box<dyn Error>> {
    let real = || async { req_get(client, url, limit).await.ok() };
    let item = CACHEDATA.load_or_store(url, real, ttl).await;
    if let Some(res) = item {
        return Ok(res);
    }
    req_get(client, url, limit).await
}

pub async fn req_get(
    client: &web::Data<Client>,
    url: &String,
    limit: u32,
) -> Result<Arc<Bytes>, Box<dyn Error>> {
    let mut response = client
        .get(url)
        .timeout(TIMEOUT)
        .insert_header(UA)
        .insert_header(AL)
        .send()
        .await?;
    let res = response.body().limit(limit as usize).await?;
    match response.status() {
        StatusCode::OK => Ok(Arc::new(res)),
        _ => {
            println!("status: failed {} {}", url, response.status());
            println!("{:?}", res);
            Err(Box::new(io::Error::other(format!(
                "{} {}",
                url,
                response.status()
            ))))
        }
    }
}
