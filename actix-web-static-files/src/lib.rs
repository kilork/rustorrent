use actix_http::body::SizedStream;
use actix_service::{NewService, Service};
use actix_web::{
    dev::{AppService, HttpServiceFactory, ResourceDef, ServiceRequest, ServiceResponse},
    error::{BlockingError, Error, ErrorInternalServerError},
    http::{
        header::{self, ContentDisposition, DispositionParam, DispositionType},
        ContentEncoding, Method, StatusCode,
    },
    HttpRequest, HttpResponse, ResponseError,
};
use failure::Fail;
use futures::{
    future::{ok, Either, FutureResult},
    Async, Future, Poll, Stream,
};
use mime::Mime;
use std::{collections::HashMap, fs::Metadata, path::PathBuf, rc::Rc};

/// Static resource files handling
///
/// `ResourceFiles` service must be registered with `App::service` method.
///
/// ```rust
/// use std::collections::HashMap;
///
/// use actix_web::App;
/// use actix_web_static_files as fs;
///
/// fn main() {
///     let files: HashMap<String, String> = vec![("./path1/file1.txt".into(), "hello".into())]
///                                             .into_iter().collect();
///     let app = App::new()
///         .service(fs::ResourceFiles::new(".", files));
/// }
/// ```
pub struct ResourceFiles {
    path: String,
    files: Rc<HashMap<String, String>>,
}

impl ResourceFiles {
    pub fn new(path: &str, files: HashMap<String, String>) -> Self {
        Self {
            path: path.into(),
            files: Rc::new(files),
        }
    }
}

impl HttpServiceFactory for ResourceFiles {
    fn register(self, config: &mut AppService) {
        let rdef = if config.is_root() {
            ResourceDef::root_prefix(&self.path)
        } else {
            ResourceDef::prefix(&self.path)
        };
        config.register_service(rdef, None, self, None)
    }
}

impl NewService for ResourceFiles {
    type Config = ();
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = Error;
    type Service = ResourceFilesService;
    type InitError = ();
    type Future = Box<dyn Future<Item = Self::Service, Error = Self::InitError>>;

    fn new_service(&self, _: &()) -> Self::Future {
        Box::new(ok(ResourceFilesService {
            files: self.files.clone(),
        }))
    }
}

pub struct ResourceFilesService {
    files: Rc<HashMap<String, String>>,
}

impl<'a> Service for ResourceFilesService {
    type Request = ServiceRequest;
    type Response = ServiceResponse;
    type Error = Error;
    type Future = Either<
        FutureResult<Self::Response, Self::Error>,
        Box<dyn Future<Item = Self::Response, Error = Self::Error>>,
    >;

    fn poll_ready(&mut self) -> Poll<(), Self::Error> {
        Ok(Async::Ready(()))
    }
    fn call(&mut self, req: ServiceRequest) -> Self::Future {
        let real_path = match get_pathbuf(req.match_info().path()) {
            Ok(item) => item,
            Err(e) => return Either::A(ok(req.error_response(e))),
        };

        let (req, _) = req.into_parts();

        Either::A(ok(match respond_to(&req, &self.files) {
            Ok(item) => ServiceResponse::new(req.clone(), item),
            Err(e) => ServiceResponse::from_err(e, req),
        }))
    }
}

fn respond_to(req: &HttpRequest, files: &HashMap<String, String>) -> Result<HttpResponse, Error> {
    match *req.method() {
        Method::HEAD | Method::GET => (),
        _ => {
            return Ok(HttpResponse::MethodNotAllowed()
                .header(header::CONTENT_TYPE, "text/plain")
                .header(header::ALLOW, "GET, HEAD")
                .body("This resource only supports GET and HEAD."));
        }
    }
    let mut resp = HttpResponse::build(StatusCode::OK);
    Ok(resp.body(format!("{:?}", files) + "\nHello, world"))
}

#[derive(Fail, Debug, PartialEq)]
pub enum UriSegmentError {
    /// The segment started with the wrapped invalid character.
    #[fail(display = "The segment started with the wrapped invalid character")]
    BadStart(char),
    /// The segment contained the wrapped invalid character.
    #[fail(display = "The segment contained the wrapped invalid character")]
    BadChar(char),
    /// The segment ended with the wrapped invalid character.
    #[fail(display = "The segment ended with the wrapped invalid character")]
    BadEnd(char),
}

/// Return `BadRequest` for `UriSegmentError`
impl ResponseError for UriSegmentError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::new(StatusCode::BAD_REQUEST)
    }
}

fn get_pathbuf(path: &str) -> Result<PathBuf, UriSegmentError> {
    let mut buf = PathBuf::new();
    for segment in path.split('/') {
        if segment == ".." {
            buf.pop();
        } else if segment.starts_with('.') {
            return Err(UriSegmentError::BadStart('.'));
        } else if segment.starts_with('*') {
            return Err(UriSegmentError::BadStart('*'));
        } else if segment.ends_with(':') {
            return Err(UriSegmentError::BadEnd(':'));
        } else if segment.ends_with('>') {
            return Err(UriSegmentError::BadEnd('>'));
        } else if segment.ends_with('<') {
            return Err(UriSegmentError::BadEnd('<'));
        } else if segment.is_empty() {
            continue;
        } else if cfg!(windows) && segment.contains('\\') {
            return Err(UriSegmentError::BadChar('\\'));
        } else {
            buf.push(segment)
        }
    }

    Ok(buf)
}

pub struct Resource {
    data: Vec<u8>,
    metadata: Metadata,
}

pub fn collect_resources() {}
