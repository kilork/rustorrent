#[macro_use]
extern crate actix_web;

#[macro_use]
extern crate serde_json;

use actix_files;
use actix_web::web;
use actix_web::{App, HttpResponse, HttpServer};
use exitfailure::ExitFailure;
use failure::ResultExt;
use handlebars::Handlebars;
use std::io;

#[get("/")]
fn index(hb: web::Data<Handlebars>) -> HttpResponse {
    let data = json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION")
    });

    let body = hb.render("index", &data).unwrap();
    HttpResponse::Ok().body(body)
}

fn main() -> Result<(), ExitFailure> {
    let mut handlebars = Handlebars::new();
    handlebars.register_templates_directory(".html", "./static/templates")?;

    let handlebars_ref = web::Data::new(handlebars);

    HttpServer::new(move || {
        App::new()
            .register_data(handlebars_ref.clone())
            .service(index)
            .service(actix_files::Files::new("/files", "./static/files").show_files_listing())
            .service(actix_files::Files::new("/css", "./static/css").show_files_listing())
    })
    .bind("127.0.0.1:8080")?
    .run()
    .map_err(|x| x.into())
}
