use super::*;

#[derive(Debug)]
pub enum RequestResponse<T, R> {
    RequestOnly(T),
    Full {
        request: T,
        response: oneshot::Sender<R>,
    },
}

impl<T: Display, R: Display> Display for RequestResponse<T, R> {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            RequestResponse::RequestOnly(_) => write!(f, "{}", self),
            RequestResponse::Full { request, .. } => {
                write!(f, "Full {{ request: {}, .. }}", request)
            }
        }
    }
}

impl<T, R> RequestResponse<T, R> {
    pub fn new(request: T) -> (Self, oneshot::Receiver<R>) {
        let (sender, receiver) = oneshot::channel();
        (
            RequestResponse::Full {
                request,
                response: sender,
            },
            receiver,
        )
    }

    pub fn request(&self) -> &T {
        match self {
            RequestResponse::RequestOnly(request) | RequestResponse::Full { request, .. } => {
                request
            }
        }
    }

    pub fn response(self, result: R) -> Result<(), RsbtError> {
        match self {
            RequestResponse::Full { response, .. } => response
                .send(result)
                .map_err(|_| RsbtError::FailureReason("Cannot send response".into())),
            RequestResponse::RequestOnly(_) => Ok(()),
        }
    }
}
