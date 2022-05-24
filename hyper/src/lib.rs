use std::task::Poll;

use anyhttp::{DynChunksStream, HttpError, Tapper};
use futures_util::{future::BoxFuture, TryFutureExt, TryStreamExt};

#[derive(Clone)]
pub struct HyperExecutor<C> {
    client: hyper::Client<C>,
}

impl<C> HyperExecutor<C>
where
    C: hyper::client::connect::Connect + Send + Sync + Clone + 'static,
{
    pub fn new(client: hyper::Client<C>) -> Self {
        Self { client }
    }

    pub fn into_client(self) -> anyhttp::Client<Self> {
        anyhttp::Client::new(self)
    }
}

impl<C> From<hyper::Client<C>> for HyperExecutor<C> {
    fn from(client: hyper::Client<C>) -> Self {
        Self { client }
    }
}

pub struct ResponseBody(pub hyper::Body);

impl anyhttp::Respond for ResponseBody {
    type Chunks = DynChunksStream;
    type BytesOutput = BoxFuture<'static, Result<Vec<u8>, anyhttp::HttpError>>;

    fn into_chunks(self) -> Self::Chunks {
        let s = self.0.map_ok(|b| b.to_vec()).map_err(|err| {
            // TODO: proper error mapping
            HttpError::new_response_read(None, err)
        });
        Box::pin(s)
    }

    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks {
        (*self).into_chunks()
    }

    fn bytes(self) -> Self::BytesOutput {
        let f = hyper::body::to_bytes(self.0)
            .map_ok(|b| b.to_vec())
            .map_err(|err| {
                // FIXME: proper error mapping
                anyhttp::HttpError::new_custom_with_cause("could not read response", Box::new(err))
            });
        Box::pin(f)
    }

    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput {
        self.bytes()
    }
}

pin_project_lite::pin_project! {
     #[project = ResponseFutureProject]
    pub enum ResponseFuture {
        Hyper { #[pin] fut: hyper::client::ResponseFuture, tap: Option<Tapper>, uri: http::Uri },
        Ready{
            res: Option<Result<anyhttp::Response<ResponseBody>, anyhttp::HttpError>>,
        }
    }
}

impl std::future::Future for ResponseFuture {
    type Output = Result<anyhttp::Response<ResponseBody>, anyhttp::HttpError>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.project() {
            ResponseFutureProject::Hyper { fut, tap, uri } => match fut.poll(cx) {
                Poll::Ready(res) => {
                    let res = res
                        .map(|res| {
                            let (parts, body) = res.into_parts();
                            let res = anyhttp::Response::from_parts(parts, ResponseBody(body));
                            let (mut res, body) = res.take_body();
                            *res.uri_mut() = uri.clone();
                            if let Some(f) = tap.take() {
                                f(&mut res);
                            }

                            res.map(move |_| body)
                        })
                        .map_err(|err| {
                            // FIXME: proper error mapping
                            anyhttp::HttpError::new_custom_with_cause("hyper error", err)
                        });

                    Poll::Ready(res)
                }
                Poll::Pending => Poll::Pending,
            },
            ResponseFutureProject::Ready { res } => {
                if let Some(res) = res.take() {
                    Poll::Ready(res)
                } else {
                    // TODO: return error here?
                    Poll::Pending
                }
            }
        }
    }
}

impl<C> anyhttp::HttpExecutor for HyperExecutor<C>
where
    C: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
{
    type RequestBody = hyper::Body;
    type ResponseBody = ResponseBody;
    type Output = ResponseFuture;

    fn request_body_from_generic(&self, body: anyhttp::RequestBody) -> Self::RequestBody {
        match body {
            anyhttp::RequestBody::Empty => hyper::Body::empty(),
            anyhttp::RequestBody::Bytes(b) => hyper::Body::from(b),
            anyhttp::RequestBody::Read(_) => todo!(),
        }
    }

    fn new_output_error(&self, error: anyhttp::HttpError) -> Self::Output {
        ResponseFuture::Ready {
            res: Some(Err(error)),
        }
    }

    fn execute(&self, pre: anyhttp::RequestPre<Self::RequestBody>) -> Self::Output {
        let uri = pre.request.uri().clone();
        let fut = self.client.request(pre.request.into());
        ResponseFuture::Hyper {
            fut,
            tap: pre.tap,
            uri,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hyper_client() {
        let exec = HyperExecutor::from(hyper::client::Client::new());
        anyhttp::test::test_async_executor(exec).await;
    }
}
