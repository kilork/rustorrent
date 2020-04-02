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
    broadcaster_sender: web::Data<Mutex<Sender<BroadcasterMessage>>>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    if let Some(item) = payload.next().await {
        let mut field = item?;
        let content_type = field.content_disposition().unwrap();
        let filename = content_type.get_filename().unwrap();

        let mut torrent = vec![];
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            torrent.extend(&data);
        }

        let (request_response, receiver) = RequestResponse::new(RsbtCommandAddTorrent {
            data: torrent,
            filename: filename.to_string(),
            state: TorrentDownloadStatus::Enabled,
        });
        {
            let mut event_sender = event_sender.lock().await;
            if let Err(err) = event_sender
                .send(RsbtCommand::AddTorrent(request_response))
                .await
            {
                error!("cannot send to torrent process: {}", err);
                return Ok(HttpResponse::InternalServerError().json(Failure {
                    error: format!("cannot send to torrent process: {}", err),
                }));
            }
        }

        return Ok(match receiver.await {
            Ok(Ok(ref torrent)) => {
                let torrent_view: TorrentDownloadView = torrent.into();
                if let Err(err) = broadcaster_sender
                    .lock()
                    .await
                    .send(BroadcasterMessage::Subscribe(torrent.clone()))
                    .await
                {
                    error!("cannot send subscribe message: {}", err);
                }
                HttpResponse::Ok().json(torrent_view)
            }
            Ok(Err(err)) => {
                error!("error in update call: {}", err);
                HttpResponse::InternalServerError().json(Failure {
                    error: format!("cannot add torrent process: {}", err),
                })
            }
            Err(err) => {
                error!("error in receiver: {}", err);
                HttpResponse::InternalServerError().json(Failure {
                    error: format!("cannot receive from add torrent process: {}", err),
                })
            }
        });
    }
    Ok(HttpResponse::UnprocessableEntity().into())
}
