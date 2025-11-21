use actix_web::web::{self, Bytes};
use awc::Client;
use std::{
    error,
    io::{self},
    sync::Arc,
};
use tokio::task;

use crate::{parser, request, util};

use super::ts;

async fn get_hls_master(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<Arc<Bytes>, Box<dyn error::Error>> {
    let url = parser::parse_url(client, vid, "hlsManifestUrl", 3600).await?;
    let data = request::req_get_cache(client, &url, 600, 2 << 20).await?;
    Ok(data)
}

pub async fn playlist_master(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<String, Box<dyn error::Error>> {
    let data = get_hls_master(client, vid).await?;
    let content = std::str::from_utf8(&data).unwrap_or_default();
    let mut uid: String = "".to_owned();
    let lines = content.lines().map(move |f| {
        if f.starts_with("#") {
            uid = util::hash(f);
            return f.to_owned() + "\r\n";
        }
        "/video/".to_owned() + vid + "/" + &uid + ".m3u8\r\n"
    });
    let text = lines.collect();
    Ok(text)
}

pub async fn playlist_index(
    client: &web::Data<Client>,
    vid: &String,
    list: &String,
) -> Result<String, Box<dyn error::Error>> {
    let data = get_hls_master(client, vid).await?;
    let content = std::str::from_utf8(&data).unwrap_or_default();
    let mut found = false;
    let item = content.lines().find(move |f| {
        if f.starts_with("#") {
            let uid = util::hash(f);
            if &uid == list {
                found = true
            }
            return false;
        }
        found
    });
    let Some(u) = item else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} {}", vid, list),
        )));
    };
    let data = request::req_get_cache(client, &u.to_string(), 5, 5 << 20).await?;
    let sub_content = std::str::from_utf8(&data).unwrap_or_default();
    let lines = sub_content.lines().map(|f| {
        if f.starts_with("#") {
            return f.to_owned() + "\r\n";
        }
        let uid = util::hash(f);
        task::spawn_local(ts::put_task(client.clone(), uid.clone(), f.to_owned()));
        format!("/video/{}/{}.ts\r\n", vid, uid)
    });
    let text = lines.collect();
    Ok(text)
}

pub async fn playlist_ts(vid: &String, ts: &String) -> Result<Arc<Bytes>, Box<dyn error::Error>> {
    let res = ts::get_task(ts).await;
    res.ok_or_else(|| -> Box<dyn error::Error> {
        Box::new(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} {}", vid, ts),
        ))
    })
}
