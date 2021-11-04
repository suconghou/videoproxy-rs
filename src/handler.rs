use crate::parser;
use actix_web::client::Client;
use actix_web::http::StatusCode;
use actix_web::{Error, HttpRequest, HttpResponse, Responder};
use std::error;
use std::io;

// 暴露的headers, 此处需要是小写
const EXPOSE_HEADERS: [&str; 7] = [
    "accept-ranges",
    "content-range",
    "content-length",
    "content-type",
    "content-encoding",
    "last-modified",
    "etag",
];

// 转发的headers, 此处需要小写
const FWD_HEADERS: [&str; 9] = [
    "user-agent",
    "accept",
    "accept-encoding",
    "accept-language",
    "if-modified-since",
    "if-none-match",
    "range",
    "content-length",
    "content-type",
];

const EXPOSE_HEADERS_SIMPLE: [&str; 5] = [
    "content-length",
    "content-type",
    "content-encoding",
    "last-modified",
    "etag",
];

const FWD_HEADERS_SIMPLE: [&str; 8] = [
    "user-agent",
    "accept",
    "accept-encoding",
    "accept-language",
    "if-modified-since",
    "if-none-match",
    "content-length",
    "content-type",
];

const CACHE_KEY: &str = "cache-control";
const CACHE_VALUE: &str = "public,max-age=864000";

const PREFER_LIST: &str =
    "18,59,22,37,243,134,396,244,135,397,247,136,302,398,248,137,242,133,395,278,598,160,597";

pub async fn get_info(vid: &String) -> Result<parser::VideoInfo, Box<dyn error::Error>> {
    parser::parse(vid).await
}

pub async fn proxy_image(req: HttpRequest, vid: &String, ext: &String) -> impl Responder {
    let url = match ext.as_str() {
        "jpg" => format!("https://i.ytimg.com/vi/{}/mqdefault.{}", vid, ext),
        _ => format!("https://i.ytimg.com/vi_webp/{}/mqdefault.{}", vid, ext),
    };
    proxy(
        req,
        url,
        10,
        Box::new(io::Error::new(io::ErrorKind::Other, "")),
    )
    .await
}

pub async fn proxy_ts(
    req: HttpRequest,
    vid: &String,
    itag: &String,
    part: &String,
) -> impl Responder {
    match get_info(vid).await {
        Ok(res) => match res.streams.get(itag) {
            Some(item) => {
                let mut url = String::new();
                url.push_str(&item.url);
                url.push_str("&range=");
                url.push_str(part);
                simple_proxy(
                    req,
                    url,
                    30,
                    Box::new(io::Error::new(io::ErrorKind::Other, "")),
                )
                .await
            }
            None => {
                simple_proxy(
                    req,
                    "".to_owned(),
                    10,
                    Box::new(io::Error::new(io::ErrorKind::NotFound, "itag not found")),
                )
                .await
            }
        },
        Err(err) => simple_proxy(req, "".to_owned(), 10, err).await,
    }
}

pub async fn proxy_file(req: HttpRequest, vid: &String, itag: &String) -> impl Responder {
    match get_info(vid).await {
        Ok(res) => match res.streams.get(itag) {
            Some(item) => {
                proxy(
                    req,
                    item.url.clone(),
                    3600,
                    Box::new(io::Error::new(io::ErrorKind::Other, "")),
                )
                .await
            }
            None => {
                proxy(
                    req,
                    "".to_owned(),
                    10,
                    Box::new(io::Error::new(io::ErrorKind::NotFound, "itag not found")),
                )
                .await
            }
        },
        Err(err) => proxy(req, "".to_owned(), 10, err).await,
    }
}

pub async fn proxy_auto(req: HttpRequest, vid: &String, prefer: &String) -> impl Responder {
    match get_info(vid).await {
        Ok(res) => match find_item(res, &prefer) {
            Some(item) => {
                proxy(
                    req,
                    item,
                    3600,
                    Box::new(io::Error::new(io::ErrorKind::Other, "")),
                )
                .await
            }
            None => {
                proxy(
                    req,
                    "".to_owned(),
                    10,
                    Box::new(io::Error::new(io::ErrorKind::NotFound, "itag not found")),
                )
                .await
            }
        },
        Err(err) => proxy(req, "".to_owned(), 10, err).await,
    }
}

async fn proxy(
    req: HttpRequest,
    url: String,
    timeout: u64,
    err: Box<dyn error::Error>,
) -> impl Responder {
    if url == "" {
        return HttpResponse::InternalServerError()
            .body(format!("{:?}", err))
            .await;
    }
    let client = Client::default();
    let mut forwarded_req = client
        .get(url)
        .no_decompress()
        .timeout(core::time::Duration::from_secs(timeout));

    let r = req.headers();
    for item in &FWD_HEADERS {
        if r.contains_key(*item) {
            forwarded_req = forwarded_req.set_header(*item, r.get(*item).unwrap().clone());
        }
    }
    forwarded_req.send().await.map_err(Error::from).map(|res| {
        let status = res.status();
        let mut client_resp = HttpResponse::build(status);
        for (header_name, header_value) in res
            .headers()
            .iter()
            .filter(|(h, _)| EXPOSE_HEADERS.contains(&h.as_str()))
        {
            client_resp.set_header(header_name.clone(), header_value.clone());
        }
        if status == StatusCode::OK {
            client_resp.set_header(CACHE_KEY, CACHE_VALUE);
        }
        client_resp.streaming(res)
    })
}

async fn simple_proxy(
    req: HttpRequest,
    url: String,
    timeout: u64,
    err: Box<dyn error::Error>,
) -> impl Responder {
    if url == "" {
        return HttpResponse::InternalServerError()
            .body(format!("{:?}", err))
            .await;
    }
    let client = Client::default();
    let mut forwarded_req = client
        .get(url)
        .no_decompress()
        .timeout(core::time::Duration::from_secs(timeout));

    let r = req.headers();
    for item in &FWD_HEADERS_SIMPLE {
        if r.contains_key(*item) {
            forwarded_req = forwarded_req.set_header(*item, r.get(*item).unwrap().clone());
        }
    }
    forwarded_req.send().await.map_err(Error::from).map(|res| {
        let status = res.status();
        let mut client_resp = HttpResponse::build(status);
        for (header_name, header_value) in res
            .headers()
            .iter()
            .filter(|(h, _)| EXPOSE_HEADERS_SIMPLE.contains(&h.as_str()))
        {
            client_resp.set_header(header_name.clone(), header_value.clone());
        }
        if status == StatusCode::OK {
            client_resp.set_header(CACHE_KEY, CACHE_VALUE);
        }
        client_resp.streaming(res)
    })
}

fn find_item(info: parser::VideoInfo, prefer: &String) -> Option<String> {
    for itag in prefer.split(',').chain(PREFER_LIST.split(',')) {
        match info.streams.get(itag) {
            Some(item) => return Some(item.url.clone()),
            None => continue,
        }
    }
    None
}
