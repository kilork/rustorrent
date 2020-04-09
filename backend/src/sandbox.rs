use super::*;

#[get("/")]
async fn sandbox() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/sandbox.html"))
}

#[get("/rsbt.mjs")]
async fn rsbt_javascript_module(_user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/javascript")
        .body(include_str!("../static/rsbt.mjs"))
}

#[get("/rsbt.css")]
async fn rsbt_css(_user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(include_str!("../static/rsbt.css"))
}

#[get("/upload")]
async fn upload_form(_user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/upload.html"))
}

#[get("/stream")]
async fn stream_page(_user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/stream.html"))
}

#[get("/torrent/{id}/piece")]
async fn torrent_piece_page(id: web::Path<usize>, _user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/piece.html").replace("{id}", &id.to_string()))
}
