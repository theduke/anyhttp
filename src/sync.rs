use std::sync::Arc;

use crate::{
    error::{self, HttpError},
    types::Response,
    HttpExecutor, RequestBody, RequestPre, Respond,
};

pub enum GenericResponseBody {
    Read(Box<dyn std::io::Read>),
}

impl Respond for GenericResponseBody {
    type Chunks = Result<Vec<u8>, HttpError>;
    type BytesOutput = Result<Vec<u8>, HttpError>;
    type Reader = Box<dyn std::io::Read>;

    fn into_chunks(self) -> Self::Chunks {
        match self {
            GenericResponseBody::Read(mut r) => {
                let mut buf = Vec::new();

                r.read_to_end(&mut buf).map_err(|err| {
                    HttpError::new_io(err, Some("could not read response body".to_string()))
                })?;

                Ok(buf)
            }
        }
    }

    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks {
        (*self).into_chunks()
    }

    fn bytes(self) -> Self::BytesOutput {
        self.into_chunks()
    }

    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput {
        (*self).bytes()
    }

    fn reader(self) -> Self::Reader {
        match self {
            GenericResponseBody::Read(r) => r,
        }
    }

    fn reader_boxed(self: Box<Self>) -> Self::Reader {
        (*self).reader()
    }
}

struct DynRespondWrapper<R>(R);

type DynChunks = Box<dyn Iterator<Item = Result<Vec<u8>, HttpError>>>;
type DynReader = Box<dyn std::io::Read>;

impl<R> Respond for DynRespondWrapper<R>
where
    R: Respond<BytesOutput = Result<Vec<u8>, HttpError>>,
    R::Chunks: Iterator<Item = Result<Vec<u8>, HttpError>> + 'static,
    R::Reader: std::io::Read + 'static,
{
    type Chunks = DynChunks;
    type BytesOutput = Result<Vec<u8>, HttpError>;
    type Reader = DynReader;

    fn into_chunks(self) -> Self::Chunks {
        Box::new(self.0.into_chunks())
    }

    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks {
        (*self).into_chunks()
    }

    fn bytes(self) -> Self::BytesOutput {
        self.0.bytes()
    }

    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput {
        (*self).0.bytes()
    }

    fn reader(self) -> Self::Reader {
        Box::new(self.0.reader())
    }

    fn reader_boxed(self: Box<Self>) -> Self::Reader {
        (*self).reader()
    }
}

struct DynWrapper<E>(E);

pub type DynResponseBody = Box<
    dyn Respond<Chunks = DynChunks, BytesOutput = Result<Vec<u8>, HttpError>, Reader = DynReader>,
>;

impl<E> HttpExecutor for DynWrapper<E>
where
    E: HttpExecutor,
    E::Output: Into<Result<Response<E::ResponseBody>, HttpError>>,
    E::ResponseBody: Respond<BytesOutput = Result<Vec<u8>, HttpError>>,
    <E::ResponseBody as Respond>::Chunks: Iterator<Item = Result<Vec<u8>, HttpError>> + 'static,
    <E::ResponseBody as Respond>::Reader: std::io::Read + 'static,
{
    type RequestBody = RequestBody;
    type ResponseBody = DynResponseBody;
    type Output = Result<Response<Self::ResponseBody>, HttpError>;

    fn request_body_from_generic(&self, body: RequestBody) -> Self::RequestBody {
        body
    }

    fn new_output_error(&self, error: HttpError) -> Self::Output {
        Err(error)
    }

    fn execute(&self, request: RequestPre<Self::RequestBody>) -> Self::Output {
        let res = self.0.execute_generic(request).into()?;
        let res = res.map_body(move |b| -> DynResponseBody { Box::new(DynRespondWrapper(b)) });
        Ok(res)
    }

    fn execute_generic(&self, pre: RequestPre<RequestBody>) -> Self::Output {
        self.execute(pre)
    }
}

pub type DynExecutor = Arc<
    dyn HttpExecutor<
        RequestBody = RequestBody,
        ResponseBody = DynResponseBody,
        Output = Result<Response<DynResponseBody>, HttpError>,
    >,
>;

pub type DynClient = super::Client<DynExecutor>;

impl<E> super::Client<E>
where
    E: HttpExecutor + 'static,
    E::ResponseBody: Respond<BytesOutput = Result<Vec<u8>, HttpError>>,
    E::Output: Into<Result<Response<E::ResponseBody>, HttpError>>,
    <E::ResponseBody as Respond>::Chunks: Iterator<Item = Result<Vec<u8>, HttpError>> + 'static,
    <E::ResponseBody as Respond>::Reader: std::io::Read + 'static,
{
    pub fn new_dyn_sync(exec: E) -> super::Client<DynExecutor> {
        let dyn_exec: DynExecutor = Arc::new(DynWrapper(exec));
        super::Client::new(dyn_exec)
    }
}

impl<B> Response<B>
where
    B: Respond<BytesOutput = Result<Vec<u8>, HttpError>>,
{
    pub fn bytes_sync(self) -> Result<Vec<u8>, HttpError> {
        self.body.bytes()
    }

    #[cfg(feature = "json")]
    pub fn json_sync<T: serde::de::DeserializeOwned>(self) -> Result<T, HttpError> {
        let bytes = self.bytes_sync()?;
        serde_json::from_slice(&bytes).map_err(|err| {
            HttpError::new(error::Kind::InvalidResponseJson, Some(Box::new(err)), None)
        })
    }
}
