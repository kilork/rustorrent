use actix_multipart::Multipart;
use actix_web::{web, Error, HttpResponse};
use futures::StreamExt;
use log::error;
use rsbt_service::{
    RsbtCommand, RsbtCommandAddTorrent, RsbtRequestResponse, RsbtTorrentDownloadView,
    RsbtTorrentProcessStatus,
};

use crate::{login::User, BroadcasterMessage, Failure};
use tokio::sync::mpsc::Sender;

#[post("/upload")]
async fn upload(
    _user: User,
    event_sender: web::Data<Sender<RsbtCommand>>,
    broadcaster_sender: web::Data<Sender<BroadcasterMessage>>,
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

        let (request_response, receiver) = RsbtRequestResponse::new(RsbtCommandAddTorrent {
            data: torrent,
            filename: filename.to_string(),
            state: RsbtTorrentProcessStatus::Enabled,
        });
        {
            let mut event_sender = event_sender.as_ref().clone();
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
                let torrent_view: RsbtTorrentDownloadView = torrent.into();
                if let Err(err) = broadcaster_sender
                    .as_ref()
                    .clone()
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
