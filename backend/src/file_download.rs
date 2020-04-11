use super::*;

struct SizedBody(usize);

impl MessageBody for SizedBody {
    fn size(&self) -> BodySize {
        BodySize::Sized(self.0)
    }

    fn poll_next(&mut self, _: &mut Context<'_>) -> Poll<Option<Result<Bytes, Error>>> {
        Poll::Ready(None)
    }
}

#[head("/torrent/{id}/file/{file_id}/download")]
async fn torrent_file_download_head(
    event_sender: web::Data<Sender<RsbtCommand>>,
    ids: web::Path<(usize, usize)>,
    _user: User,
) -> impl Responder {
    let (id, file_id) = *ids;
    let torrent_file = torrent_command_result(
        event_sender,
        RsbtCommandTorrentFileDownload { id, file_id },
        RsbtCommand::TorrentFile,
    )
    .await;
    match torrent_file {
        Ok(torrent_file) => HttpResponse::Ok()
            .set_header(
                http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", torrent_file.name),
            )
            .set_header(http::header::ACCEPT_RANGES, "bytes")
            .body(Body::from_message(SizedBody(torrent_file.size))),
        Err(RsbtError::TorrentNotFound(_)) | Err(RsbtError::TorrentFileNotFound(_)) => {
            HttpResponse::NotFound().finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[get("/torrent/{id}/file/{file_id}/download")]
async fn torrent_file_download(
    event_sender: web::Data<Sender<RsbtCommand>>,
    ids: web::Path<(usize, usize)>,
    _user: User,
) -> impl Responder {
    let (id, file_id) = *ids;
    let download_stream = torrent_command_result(
        event_sender,
        RsbtCommandTorrentFileDownload { id, file_id },
        RsbtCommand::TorrentFileDownload,
    )
    .await;

    match download_stream {
        Ok(download_stream) => HttpResponse::Ok()
            .keep_alive()
            .no_chunking()
            .set_header(
                http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", download_stream.name),
            )
            .content_length(download_stream.size as u64)
            .streaming(download_stream.map_err(|x| {
                actix_web::Error::from(HttpResponse::InternalServerError().json(Failure {
                    error: format!("{}", x),
                }))
            })),
        Err(err @ RsbtError::TorrentNotFound(_)) | Err(err @ RsbtError::TorrentFileNotFound(_)) => {
            HttpResponse::NotFound().json(Failure {
                error: format!("{}", err),
            })
        }
        Err(err) => HttpResponse::InternalServerError().json(Failure {
            error: format!("{}", err),
        }),
    }
}
