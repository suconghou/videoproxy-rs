use crate::request;
use actix_web::web;
use awc::Client;
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use std::io;

#[derive(Serialize, Debug)]
pub struct StreamItem {
    quality: String,
    r#type: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    itag: String,
    len: String,
    #[serde(rename = "initRange")]
    #[serde(skip_serializing_if = "Option::is_none")]
    init_range: Option<serde_json::Map<std::string::String, serde_json::Value>>,
    #[serde(rename = "indexRange")]
    #[serde(skip_serializing_if = "Option::is_none")]
    index_range: Option<serde_json::Map<std::string::String, serde_json::Value>>,
}

#[derive(Serialize, Debug)]
pub struct VideoInfo {
    id: String,
    title: String,
    duration: String,
    author: String,
    live: bool,
    pub streams: HashMap<String, StreamItem>,
}

impl VideoInfo {
    pub fn clean(mut self) -> VideoInfo {
        for (_, val) in self.streams.iter_mut() {
            val.url = "".to_owned();
        }
        self
    }
}

pub async fn parse(client: &web::Data<Client>, vid: &String) -> Result<VideoInfo, Box<dyn Error>> {
    let res = request::getplayer(client, &vid).await?;
    let status = res["playabilityStatus"]["status"].as_str().unwrap_or("");
    if status != "OK" {
        let reason = res["playabilityStatus"]["reason"]
            .as_str()
            .unwrap_or(status);
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("{} {}", vid, reason),
        )));
    }
    let stream_items: HashMap<String, StreamItem> = HashMap::new();
    let mut info = VideoInfo {
        id: (&*vid).to_owned(),
        title: res["videoDetails"]["title"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        duration: res["videoDetails"]["lengthSeconds"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        author: res["videoDetails"]["author"]
            .as_str()
            .unwrap_or("")
            .to_owned(),
        live: res["videoDetails"]["isLive"].as_bool().unwrap_or_default(),
        streams: stream_items,
    };
    let mut streams: Vec<serde_json::Value> = [].to_vec();
    if let Some(video_info_itags) = res["streamingData"]["formats"].as_array() {
        streams = [streams, video_info_itags.to_vec()].concat();
    }
    if let Some(video_info_itags_adaptive) = res["streamingData"]["adaptiveFormats"].as_array() {
        streams = [streams, video_info_itags_adaptive.to_vec()].concat();
    }
    for item in streams {
        let i = item["itag"].as_u64().unwrap_or(0);
        let itag = i.to_string();
        let itags = i.to_string();
        let url = item["url"].as_str().unwrap_or("");
        let len = item["contentLength"].as_str().unwrap_or("");
        let mime = item["mimeType"].as_str().unwrap_or("");
        let quality = item["qualityLabel"]
            .as_str()
            .unwrap_or_else(|| item["quality"].as_str().unwrap_or(""));
        info.streams.insert(
            itag,
            StreamItem {
                quality: quality.to_owned(),
                len: len.to_owned(),
                itag: itags,
                url: url.to_owned(),
                r#type: mime.to_owned(),
                init_range: item["initRange"].as_object().map(|v| v.clone()),
                index_range: item["indexRange"].as_object().map(|v| v.clone()),
            },
        );
    }
    Ok(info)
}

pub async fn parse_url(
    client: &web::Data<Client>,
    vid: &String,
    key: &str,
) -> Result<String, Box<dyn Error>> {
    let res = request::getplayer(client, &vid).await?;
    let status = res["playabilityStatus"]["status"].as_str().unwrap_or("");
    if status != "OK" {
        let reason = res["playabilityStatus"]["reason"]
            .as_str()
            .unwrap_or(status);
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("{} {}", vid, reason),
        )));
    };
    let Some(url) = res["streamingData"][key].as_str() else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} {}", vid, key),
        )));
    };
    return Ok(url.to_owned());
}
