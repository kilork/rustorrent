use super::*;

pub enum RequestResponse<T, R> {
    RequestOnly(T),
    ResponseOnly(oneshot::Sender<R>),
    Full {
        request: T,
        response: oneshot::Sender<R>,
    },
}

impl<T, R> RequestResponse<T, R> {
    pub fn request(&self) -> Option<&T> {
        match self {
            RequestResponse::RequestOnly(request) | RequestResponse::Full { request, .. } => {
                Some(request)
            }
            RequestResponse::ResponseOnly(_) => None,
        }
    }

    pub fn response(self, result: R) -> Result<(), RsbtError> {
        match self {
            RequestResponse::ResponseOnly(response) | RequestResponse::Full { response, .. } => {
                response
                    .send(result)
                    .map_err(|_| RsbtError::FailureReason("Cannot send response".into()))
            }
            RequestResponse::RequestOnly(_) => Ok(()),
        }
    }
}
