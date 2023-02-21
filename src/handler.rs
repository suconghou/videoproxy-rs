use crate::parser;
use actix_web::http::header::CACHE_CONTROL;
use actix_web::http::StatusCode;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use awc::Client;
use awc::ClientRequest;
use core::time::Duration;
use std::error;
use std::io::{Error, ErrorKind};

// 暴露的headers, 此处需要是小写
const EXPOSE: &[&str] = &[
    "accept-ranges",
    "content-range",
    "content-length",
    "content-type",
    "content-encoding",
    "last-modified",
    "etag",
];

// 转发的headers, 此处需要小写
const FWD: &[&str] = &[
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

const EXPOSE_SIMPLE: &[&str] = &[
    "content-length",
    "content-type",
    "content-encoding",
    "last-modified",
    "etag",
];

const FWD_SIMPLE: &[&str] = &[
    "user-agent",
    "accept",
    "accept-encoding",
    "accept-language",
    "if-modified-since",
    "if-none-match",
    "content-length",
    "content-type",
];

const PREFER_LIST: &str =
    "18,59,22,37,243,134,396,244,135,397,247,136,302,398,248,137,242,133,395,278,598,160,597";

pub async fn get_info(
    client: &web::Data<Client>,
    vid: &String,
) -> Result<parser::VideoInfo, Box<dyn error::Error>> {
    parser::parse(client, vid).await
}

pub async fn proxy_image(
    client: web::Data<Client>,
    req: HttpRequest,
    vid: &String,
    ext: &String,
) -> impl Responder {
    let url = match ext.as_str() {
        "jpg" => format!("https://i.ytimg.com/vi/{}/mqdefault.{}", vid, ext),
        _ => format!("https://i.ytimg.com/vi_webp/{}/mqdefault.{}", vid, ext),
    };
    proxy(client, req, url, 10, None).await
}

pub async fn proxy_ts(
    client: web::Data<Client>,
    req: HttpRequest,
    vid: &String,
    itag: &String,
    part: &String,
) -> impl Responder {
    match get_info(&client, vid).await {
        Ok(res) => match res.streams.get(itag) {
            Some(item) => {
                let mut url = String::new();
                url.push_str(&item.url);
                url.push_str("&range=");
                url.push_str(part);
                simple_proxy(client, req, url, 30, None).await
            }
            None => {
                simple_proxy(
                    client,
                    req,
                    "".to_owned(),
                    10,
                    Some(Box::new(Error::new(ErrorKind::NotFound, "itag not found"))),
                )
                .await
            }
        },
        Err(err) => simple_proxy(client, req, "".to_owned(), 10, Some(err)).await,
    }
}

pub async fn proxy_file(
    client: web::Data<Client>,
    req: HttpRequest,
    vid: &String,
    itag: &String,
) -> impl Responder {
    match get_info(&client, vid).await {
        Ok(res) => match res.streams.get(itag) {
            Some(item) => proxy(client, req, item.url.clone(), 3600, None).await,
            None => {
                proxy(
                    client,
                    req,
                    "".to_owned(),
                    10,
                    Some(Box::new(Error::new(ErrorKind::NotFound, "itag not found"))),
                )
                .await
            }
        },
        Err(err) => proxy(client, req, "".to_owned(), 10, Some(err)).await,
    }
}

pub async fn proxy_auto(
    client: web::Data<Client>,
    req: HttpRequest,
    vid: &String,
    prefer: &String,
) -> impl Responder {
    match get_info(&client, vid).await {
        Ok(res) => match find_item(res, &prefer) {
            Some(item) => proxy(client, req, item, 3600, None).await,
            None => {
                proxy(
                    client,
                    req,
                    "".to_owned(),
                    10,
                    Some(Box::new(Error::new(ErrorKind::NotFound, "itag not found"))),
                )
                .await
            }
        },
        Err(err) => proxy(client, req, "".to_owned(), 10, Some(err)).await,
    }
}

async fn proxy(
    client: web::Data<Client>,
    req: HttpRequest,
    url: String,
    timeout: u64,
    err: Option<Box<dyn error::Error>>,
) -> impl Responder {
    base_proxy(client, req, url, timeout, err, FWD, EXPOSE).await
}

async fn simple_proxy(
    client: web::Data<Client>,
    req: HttpRequest,
    url: String,
    timeout: u64,
    err: Option<Box<dyn error::Error>>,
) -> impl Responder {
    base_proxy(client, req, url, timeout, err, FWD_SIMPLE, EXPOSE_SIMPLE).await
}

async fn base_proxy(
    client: web::Data<Client>,
    req: HttpRequest,
    url: String,
    timeout: u64,
    err: Option<Box<dyn error::Error>>,
    forward_headers: &'static [&str],
    expose_headers: &'static [&str],
) -> impl Responder {
    if let Some(err) = err {
        return HttpResponse::InternalServerError().body(format!("{:?}", err));
    }
    let mut forwarded_req = request(client, url, timeout);

    let r = req.headers();
    for item in forward_headers {
        if r.contains_key(*item) {
            forwarded_req = forwarded_req.insert_header((*item, r.get(*item).unwrap().clone()));
        }
    }
    let res = forwarded_req.send().await;
    let Ok(res) = res else {
        return HttpResponse::InternalServerError().body(format!("{:?}", res.err().unwrap()));
    };
    let status = res.status();
    let mut client_resp = HttpResponse::build(status);
    for (header_name, header_value) in res
        .headers()
        .iter()
        .filter(|(h, _)| expose_headers.contains(&h.as_str()))
    {
        client_resp.insert_header((header_name, header_value.clone()));
    }
    if status == StatusCode::OK {
        client_resp.insert_header((CACHE_CONTROL, "public,max-age=86400"));
    }
    client_resp.streaming(res)
}

#[inline]
fn request(client: web::Data<Client>, url: String, timeout: u64) -> ClientRequest {
    client
        .get(url)
        .no_decompress()
        .timeout(Duration::from_secs(timeout))
}

#[inline]
fn find_item(info: parser::VideoInfo, prefer: &String) -> Option<String> {
    for itag in prefer.split(',').chain(PREFER_LIST.split(',')) {
        let Some(item) = info.streams.get(itag) else {
            continue;
        };
        return Some(item.url.clone());
    }
    None
}
