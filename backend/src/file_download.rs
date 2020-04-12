use super::*;
use std::ops::Range;

struct SizedBody(usize);

impl MessageBody for SizedBody {
    fn size(&self) -> BodySize {
        BodySize::Sized(self.0)
    }

    fn poll_next(&mut self, _: &mut Context<'_>) -> Poll<Option<Result<Bytes, Error>>> {
        Poll::Ready(None)
    }
}

const RANGE_BYTES_PREFIX: &str = "bytes=";

fn range(request: &HttpRequest) -> Option<Range<usize>> {
    if let Some(range_header) = request
        .headers()
        .get(&http::header::RANGE)
        .map(|x| x.to_str().ok())
        .flatten()
        .filter(|x| x.starts_with(RANGE_BYTES_PREFIX))
        .map(|x| &x[RANGE_BYTES_PREFIX.len()..])
    {
        if let [Ok(start), Ok(end)] = range_header
            .split('-')
            .map(&str::parse)
            .collect::<Vec<Result<usize, _>>>()
            .as_slice()
        {
            return Some(Range {
                start: *start,
                end: end + 1,
            });
        }
    }
    None
}

#[head("/torrent/{id}/file/{file_id}/download")]
async fn torrent_file_download_head(
    request: HttpRequest,
    event_sender: web::Data<Sender<RsbtCommand>>,
    ids: web::Path<(usize, usize)>,
    _user: User,
) -> impl Responder {
    let (id, file_id) = *ids;
    let torrent_file = torrent_command_result(
        event_sender,
        RsbtCommandTorrentFileDownload {
            id,
            file_id,
            range: range(&request),
        },
        RsbtCommand::TorrentFileDownloadHeader,
    )
    .await;
    match torrent_file {
        Ok(torrent_file) => HttpResponse::Ok()
            .set(http::header::ContentDisposition {
                disposition: http::header::DispositionType::Attachment,
                parameters: vec![http::header::DispositionParam::Filename(torrent_file.name)],
            })
            .set_header(http::header::ACCEPT_RANGES, "bytes")
            .body(Body::from_message(SizedBody(torrent_file.size))),
        Err(RsbtError::TorrentNotFound(_)) | Err(RsbtError::TorrentFileNotFound(_)) => {
            HttpResponse::NotFound().finish()
        }
        Err(RsbtError::TorrentFileRangeInvalid { file_size }) => {
            HttpResponse::RangeNotSatisfiable()
                .header(
                    http::header::CONTENT_RANGE,
                    format!("bytes: */{}", file_size),
                )
                .finish()
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

#[get("/torrent/{id}/file/{file_id}/download")]
async fn torrent_file_download(
    request: HttpRequest,
    event_sender: web::Data<Sender<RsbtCommand>>,
    ids: web::Path<(usize, usize)>,
    _user: User,
) -> impl Responder {
    let (id, file_id) = *ids;
    let download_stream = torrent_command_result(
        event_sender,
        RsbtCommandTorrentFileDownload {
            id,
            file_id,
            range: range(&request),
        },
        RsbtCommand::TorrentFileDownload,
    )
    .await;

    match download_stream {
        Ok(download_stream) => {
            let mut response = if download_stream.range.is_some() {
                HttpResponse::PartialContent()
            } else {
                HttpResponse::Ok()
            };
            if let Some(Range { start, end }) = download_stream.range {
                response.set_header(
                    http::header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end - 1, download_stream.file_size),
                );
            }
            response
                .keep_alive()
                .no_chunking()
                .set(http::header::ContentDisposition {
                    disposition: http::header::DispositionType::Attachment,
                    parameters: vec![http::header::DispositionParam::Filename(
                        download_stream.name.clone(),
                    )],
                })
                .set_header(http::header::ACCEPT_RANGES, "bytes")
                .content_length(download_stream.size as u64)
                .streaming(download_stream.map_err(|x| {
                    actix_web::Error::from(HttpResponse::InternalServerError().json(Failure {
                        error: format!("{}", x),
                    }))
                }))
        }
        Err(err @ RsbtError::TorrentNotFound(_)) | Err(err @ RsbtError::TorrentFileNotFound(_)) => {
            HttpResponse::NotFound().json(Failure {
                error: format!("{}", err),
            })
        }
        Err(RsbtError::TorrentFileRangeInvalid { file_size }) => {
            HttpResponse::RangeNotSatisfiable()
                .header(
                    http::header::CONTENT_RANGE,
                    format!("bytes: */{}", file_size),
                )
                .finish()
        }
        Err(err) => HttpResponse::InternalServerError().json(Failure {
            error: format!("{}", err),
        }),
    }
}
