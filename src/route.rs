use crate::cache::map::{CACHEDATA, CACHEJSON};
use crate::handler;
use crate::hls::{playlist, ts};
use actix_files as fs;
use actix_web::http::header::CACHE_CONTROL;
use actix_web::{get, post, web, Error, HttpRequest, HttpResponse, Responder, Result};
use awc::Client;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct Quality {
    prefer: Option<String>,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[get("/video/{vid:[\\w\\-]{6,15}}.{ext:(json)}")]
async fn vinfo(info: web::Path<(String, String)>, client: web::Data<Client>) -> impl Responder {
    let info = info.into_inner();
    match handler::get_info(&client, &info.0).await {
        Ok(res) => HttpResponse::Ok()
            .insert_header((
                CACHE_CONTROL,
                format!("public,max-age=3600{}", CACHEJSON.len().await),
            ))
            .json(res.clean()),
        Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
    }
}

#[get("/video/{vid:[\\w\\-]{6,15}}.{ext:(m3u8)}")]
async fn hls(info: web::Path<(String, String)>, client: web::Data<Client>) -> impl Responder {
    let info = info.into_inner();
    match playlist::playlist_master(&client, &info.0).await {
        Ok(res) => HttpResponse::Ok()
            .content_type("application/vnd.apple.mpegurl")
            .insert_header((
                CACHE_CONTROL,
                format!("public,max-age={}", CACHEDATA.len().await),
            ))
            .body(res),
        Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
    }
}

#[get("/video/{vid:[\\w\\-]{6,15}}/{list:[\\w]{1,8}}.{ext:(m3u8)}")]
async fn hls_list(info: web::Path<(String, String)>, client: web::Data<Client>) -> impl Responder {
    let info = info.into_inner();
    match playlist::playlist_index(&client, &info.0, &info.1).await {
        Ok(res) => HttpResponse::Ok()
            .content_type("application/vnd.apple.mpegurl")
            .insert_header((
                CACHE_CONTROL,
                format!("public,max-age={}", ts::thread().await),
            ))
            .body(res),
        Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
    }
}

#[get("/video/{vid:[\\w\\-]{6,15}}/{uid:[\\w]{1,8}}.{ext:(ts)}")]
async fn hls_ts(info: web::Path<(String, String)>) -> impl Responder {
    let info = info.into_inner();
    match playlist::playlist_ts(&info.0, &info.1).await {
        Ok(res) => HttpResponse::Ok()
            .insert_header((CACHE_CONTROL, "public,max-age=3600"))
            .body(res.slice(..)),
        Err(err) => HttpResponse::InternalServerError().body(format!("{:?}", err)),
    }
}

#[get("/video/{vid:[\\w\\-]{6,15}}.{ext:(jpg|webp)}")]
async fn image(
    req: HttpRequest,
    info: web::Path<(String, String)>,
    client: web::Data<Client>,
) -> impl Responder {
    let info = info.into_inner();
    handler::proxy_image(client, req, &info.0, &info.1).await
}

#[get("/video/{vid:[\\w\\-]{6,15}}/{itag:\\d+}.{ext:(webm|mp4)}")]
async fn stream(
    req: HttpRequest,
    info: web::Path<(String, String, String)>,
    client: web::Data<Client>,
) -> impl Responder {
    let info = info.into_inner();
    handler::proxy_file(client, req, &info.0, &info.1).await
}

#[get("/video/{vid:[\\w\\-]{6,15}}/{itag:\\d+}/{range:\\d+-\\d+}.ts")]
async fn streamts(
    req: HttpRequest,
    info: web::Path<(String, String, String)>,
    client: web::Data<Client>,
) -> impl Responder {
    let info = info.into_inner();
    handler::proxy_ts(client, req, &info.0, &info.1, &info.2).await
}

#[get("/video/{vid:[\\w\\-]{6,15}}.{ext:(webm|mp4)}")]
async fn streamauto(
    req: HttpRequest,
    params: web::Query<Quality>,
    info: web::Path<(String, String)>,
    client: web::Data<Client>,
) -> impl Responder {
    let info = info.into_inner();
    handler::proxy_auto(
        client,
        req,
        &info.0,
        params.prefer.as_ref().unwrap_or(&"".to_owned()),
    )
    .await
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

pub async fn serve(req: HttpRequest) -> Result<fs::NamedFile, Error> {
    let path: PathBuf = req.match_info().query("filename").parse()?;
    let file = fs::NamedFile::open(path)?;
    Ok(file
        .disable_content_disposition()
        .use_last_modified(true)
        .use_etag(true)
        .prefer_utf8(true))
}
