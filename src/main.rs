#[macro_use]
extern crate lazy_static;

extern crate tokio;

mod handler;
mod parser;
mod request;
mod route;
mod util;
mod cache {
    pub mod map;
}
mod hls {
    pub mod playlist;
    pub mod ts;
}

use actix_files as fs;
use actix_web::http::header::ACCESS_CONTROL_ALLOW_ORIGIN;
use actix_web::web::Data;
use actix_web::{middleware, web, App, HttpServer};
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let (addr, mount_path, serve_from) = opt();
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(awc::Client::new()))
            .wrap(middleware::DefaultHeaders::new().add((ACCESS_CONTROL_ALLOW_ORIGIN, "*")))
            .service(route::hello)
            .service(route::echo)
            .service(route::vinfo)
            .service(route::image)
            .service(route::stream)
            .service(route::streamts)
            .service(route::streamauto)
            .service(route::hls)
            .service(route::hls_list)
            .service(route::hls_ts)
            .service(
                fs::Files::new(&mount_path, serve_from.clone())
                    .show_files_listing()
                    .disable_content_disposition()
                    .prefer_utf8(true)
                    .use_last_modified(true)
                    .use_etag(true),
            )
            .route("/{filename:.*\\.\\w{1,4}}", web::get().to(route::serve))
    })
    .bind(addr)?
    .run()
    .await
}

fn opt() -> (String, String, String) {
    let mut opts: Vec<String> = vec![
        env::var("ADDR").unwrap_or("127.0.0.1:8080".to_owned()),
        env::var("PUBLIC_PATH").unwrap_or("/public".to_owned()),
        env::var("PUBLIC_DIR").unwrap_or("public".to_owned()),
    ];
    let mut index = 0;
    let mut first = true;
    for argument in env::args() {
        if first {
            first = false;
            continue;
        }
        if index >= 3 {
            break;
        }
        opts[index] = argument;
        index = index + 1;
    }
    (opts[0].clone(), opts[1].clone(), opts[2].clone())
}
