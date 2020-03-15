use super::*;

#[get("/upload")]
async fn upload_form(_user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/upload.html"))
}

#[post("/upload")]
async fn upload(
    _user: User,
    event_sender: web::Data<Mutex<Sender<RsbtCommand>>>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    while let Some(item) = payload.next().await {
        let mut field = item?;
        let content_type = field.content_disposition().unwrap();
        let filename = content_type.get_filename().unwrap();

        let mut torrent = vec![];
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            torrent.extend(&data);
        }

        let mut event_sender = event_sender.lock().await;
        if let Err(err) = event_sender
            .send(RsbtCommand::AddTorrent(
                RequestResponse::RequestOnly(torrent),
                filename.to_string(),
            ))
            .await
        {
            error!("cannot send to torrent process: {}", err);
            return Ok(HttpResponse::InternalServerError().json(Failure {
                error: format!("cannot send to torrent process: {}", err),
            }));
        }
    }
    Ok(HttpResponse::Ok().into())
}
