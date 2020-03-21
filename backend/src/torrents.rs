use super::*;

struct Paging {
    page: Option<usize>,
    size: Option<usize>,
    sort: Vec<String>,
}

impl FromRequest for Paging {
    type Config = ();
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Paging, Error>>>>;

    fn from_request(req: &HttpRequest, _pl: &mut Payload) -> Self::Future {
        let query_string = req.query_string().as_bytes().to_vec();

        Box::pin(async move {
            let mut page = None;
            let mut size = None;
            let mut sort = vec![];
            for (key, value) in url::form_urlencoded::parse(&query_string).into_owned() {
                match key.as_str() {
                    "page" => match value.parse() {
                        Ok(page_value) => page = Some(page_value),
                        Err(err) => {
                            return Err(actix_web::error::ErrorUnprocessableEntity(format!(
                                "{}",
                                err
                            )))
                        }
                    },
                    "size" => match value.parse() {
                        Ok(size_value) => size = Some(size_value),
                        Err(err) => {
                            return Err(actix_web::error::ErrorUnprocessableEntity(format!(
                                "{}",
                                err
                            )))
                        }
                    },
                    "sort" => {
                        sort.push(value);
                    }
                    other => {
                        return Err(actix_web::error::ErrorUnprocessableEntity(format!(
                            "Unexpected key: {}",
                            other
                        )))
                    }
                }
            }
            Ok(Paging { page, size, sort })
        })
    }
}

#[get("/torrents")]
async fn torrent_list(
    paging: Paging,
    event_sender: web::Data<Mutex<Sender<RsbtCommand>>>,
    _user: User,
) -> impl Responder {
    let (sender, receiver) = oneshot::channel();

    {
        let mut event_sender = event_sender.lock().await;
        if let Err(err) = event_sender.send(RsbtCommand::TorrentList { sender }).await {
            error!("cannot send to torrent process: {}", err);
            return HttpResponse::InternalServerError().json(Failure {
                error: format!("cannot send to torrent process: {}", err),
            });
        }
    }

    match receiver.await {
        Ok(torrents) => {
            let mut torrents: Vec<_> = torrents
                .iter()
                .map(|torrent| BackendTorrentDownload {
                    id: torrent.id,
                    name: torrent.name.as_str().into(),
                    received: 0,
                    uploaded: 0,
                    length: torrent.process.info.length,
                    active: true,
                })
                .collect();
            {
                type TD<'a> = &'a BackendTorrentDownload<'a>;
                let mut fields_order: Box<dyn Fn(TD, TD) -> Ordering> =
                    Box::new(|_, _| Ordering::Equal);
                let mut sorted_fields = paging
                    .sort
                    .iter()
                    .map(|x| x.split(','))
                    .map(|mut x| (x.next(), x.next()));

                let id_comparator = |a: TD, b: TD| a.id.cmp(&b.id);
                let name_comparator = |a: TD, b: TD| a.name.cmp(&b.name);

                while let Some((Some(field), order)) = sorted_fields.next() {
                    info!("order by field {} {:?}", field, order);

                    let mut field_order: Box<dyn Fn(TD, TD) -> Ordering> = match field {
                        "id" => Box::new(id_comparator),
                        "name" => Box::new(name_comparator),
                        _ => panic!(),
                    };

                    let descending = order == Some("desc");
                    if descending {
                        field_order = Box::new(move |a, b| field_order(a, b).reverse());
                    }

                    fields_order = Box::new(move |a, b| fields_order(a, b).then(field_order(a, b)));
                }

                torrents.sort_by(|a, b| fields_order(a, b));
            }

            let page = paging.page.unwrap_or_default();
            let size = paging.size.unwrap_or(20);

            HttpResponse::Ok()
                .json::<Vec<_>>(torrents.iter().skip(page * size).take(size).collect())
        }
        Err(err) => {
            error!("error in receiver: {}", err);
            HttpResponse::InternalServerError().json(Failure {
                error: format!("cannot receive from torrent process: {}", err),
            })
        }
    }
}
