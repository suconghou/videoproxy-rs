use actix_web::web::{self, Bytes};
use awc::Client;
use std::{
    error,
    io::{self, BufRead},
    sync::Arc,
};
use tokio::task;

use crate::{parser, request, util};

use super::ts;

async fn get_hls_master(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<Arc<Bytes>, Box<dyn error::Error>> {
    let url = parser::parse_url(client, vid, "hlsManifestUrl").await?;
    let data = request::getdata(client, &url, 600, 2 << 20).await?;
    Ok(data)
}

pub async fn playlist_master(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<String, Box<dyn error::Error>> {
    let data = get_hls_master(client, vid).await?;
    let mut uid: String = "".to_owned();
    let lines = data.lines().map(|f| f.unwrap()).map(move |f| {
        if f.starts_with("#") {
            uid = util::hash(&f);
            return f + "\r\n";
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
    let mut found = false;
    let item = data.lines().map(|f| f.unwrap()).find(move |f| {
        if f.starts_with("#") {
            let uid = util::hash(&f);
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
        )))
    };
    let data = request::getdata(client, &u, 5, 5 << 20).await?;
    let lines = data.lines().map(|f| f.unwrap()).map(|f| {
        if f.starts_with("#") {
            return f + "\r\n";
        }
        let uid = util::hash(&f);
        task::spawn_local(ts::put_task(client.clone(), uid.clone(), f.clone()));
        "/video/".to_owned() + vid + "/" + &uid + ".ts\r\n"
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
