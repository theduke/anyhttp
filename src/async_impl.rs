use std::{future::Future, pin::Pin, sync::Arc};

use futures::{stream::BoxStream, Stream, TryFutureExt};

use crate::{error::HttpError, HttpExecutor, RequestBody, RequestPre, Respond, Response};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
pub type HttpFuture<'a, T> = BoxFuture<'a, Result<T, HttpError>>;

pub type DynChunksStream = BoxStream<'static, Result<Vec<u8>, HttpError>>;

pub type DynReader = Pin<Box<dyn futures::io::AsyncRead + Send>>;

struct ResponseWrap<R>(R);

impl<R> Respond for ResponseWrap<R>
where
    R: Respond,
    <R as Respond>::BytesOutput: Future<Output = Result<Vec<u8>, HttpError>> + Send + 'static,
    <R as Respond>::Chunks: Stream<Item = Result<Vec<u8>, HttpError>> + Send + 'static,
    <R as Respond>::Reader: futures::io::AsyncRead + Send + 'static,
{
    type Chunks = DynChunksStream;
    type BytesOutput = HttpFuture<'static, Vec<u8>>;
    type Reader = DynReader;

    fn into_chunks(self) -> Self::Chunks {
        Box::pin(self.0.into_chunks())
    }

    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks {
        self.into_chunks()
    }

    fn bytes(self) -> Self::BytesOutput {
        Box::pin(self.0.bytes())
    }

    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput {
        Box::pin(self.0.bytes())
    }

    fn reader(self) -> Self::Reader {
        Box::pin(self.0.reader())
    }

    fn reader_boxed(self: Box<Self>) -> Self::Reader {
        self.reader()
    }
}

struct DynWrapper<E>(E);

pub type DynResponseBody = Box<
    dyn Respond<
            BytesOutput = HttpFuture<'static, Vec<u8>>,
            Chunks = DynChunksStream,
            Reader = Pin<Box<dyn futures::io::AsyncRead + Send>>,
        > + Send,
>;

// impl Respond for DynResponseBody {
//     type Chunks = DynChunksStream;
//     type BytesOutput = HttpFuture<'static, Vec<u8>>;

//     fn into_chunks(self) -> Self::Chunks {
//         self.into_chunks_boxed()
//     }

//     fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks {
//         self.into_chunks()
//     }

//     fn bytes(self) -> Self::BytesOutput {
//         self.bytes_boxed()
//     }

//     fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput {
//         self.bytes()
//     }
// }

impl<E> HttpExecutor for DynWrapper<E>
where
    E: HttpExecutor + 'static,
    E::ResponseBody: Respond + Send + 'static,
    <E::ResponseBody as Respond>::BytesOutput:
        Future<Output = Result<Vec<u8>, HttpError>> + Send + 'static,
    <E::ResponseBody as Respond>::Chunks:
        Stream<Item = Result<Vec<u8>, HttpError>> + Send + 'static,
    <E::ResponseBody as Respond>::Reader: futures::io::AsyncRead + Send + 'static,
    E::Output:
        std::future::Future<Output = Result<Response<E::ResponseBody>, HttpError>> + Send + 'static,
{
    type RequestBody = RequestBody;
    type ResponseBody = DynResponseBody;
    type Output = BoxFuture<'static, Result<Response<DynResponseBody>, HttpError>>;

    fn request_body_from_generic(&self, body: RequestBody) -> Self::RequestBody {
        body
    }

    fn execute(&self, request: RequestPre<Self::RequestBody>) -> Self::Output {
        let f = self
            .0
            .execute_generic(request)
            .and_then(move |res| async move {
                let res = res.map_body(move |b| -> DynResponseBody { Box::new(ResponseWrap(b)) });
                Ok(res)
            });

        Box::pin(f)
    }

    fn execute_generic(&self, pre: RequestPre<RequestBody>) -> Self::Output {
        self.execute(pre)
    }

    fn new_output_error(&self, error: HttpError) -> Self::Output {
        Box::pin(std::future::ready(Err(error)))
    }
}

pub type DynExecutor = Arc<
    dyn HttpExecutor<
        RequestBody = RequestBody,
        ResponseBody = DynResponseBody,
        Output = BoxFuture<'static, Result<Response<DynResponseBody>, HttpError>>,
    >,
>;

impl<E> super::Client<E>
where
    E: HttpExecutor + 'static,
    E::ResponseBody: Respond + Send + 'static,
    <E::ResponseBody as Respond>::BytesOutput:
        Future<Output = Result<Vec<u8>, HttpError>> + Send + 'static,
    <E::ResponseBody as Respond>::Chunks:
        Stream<Item = Result<Vec<u8>, HttpError>> + Send + 'static,
    <E::ResponseBody as Respond>::Reader: futures::io::AsyncRead + Send + 'static,
    E::Output:
        std::future::Future<Output = Result<Response<E::ResponseBody>, HttpError>> + Send + 'static,
{
    pub fn new_dyn_async(exec: E) -> super::Client<DynExecutor> {
        let dyn_exec: DynExecutor = Arc::new(DynWrapper(exec));
        super::Client::new(dyn_exec)
    }
}

impl<B> Response<B>
where
    B: Respond,
    <B as Respond>::BytesOutput: Future<Output = Result<Vec<u8>, HttpError>> + Send + 'static,
    <B as Respond>::Chunks: Future<Output = Result<Vec<u8>, HttpError>> + Send + 'static,
{
    pub async fn bytes_async(self) -> Result<Vec<u8>, HttpError> {
        self.body.bytes().await
    }

    #[cfg(feature = "json")]
    pub async fn json_async<T: serde::de::DeserializeOwned>(self) -> Result<T, HttpError> {
        let bytes = self.bytes_async().await?;
        serde_json::from_slice(&bytes).map_err(|err| {
            HttpError::new(
                crate::error::Kind::InvalidResponseJson,
                Some(Box::new(err)),
                None,
            )
        })
    }
}

pub type DynClient = super::Client<DynExecutor>;
