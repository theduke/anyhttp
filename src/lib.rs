mod builder;
mod error;
mod types;

#[cfg(feature = "test")]
pub mod test;

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "async")]
mod async_impl;

use std::sync::Arc;

pub use http::{
    header::{self, HeaderName, HeaderValue},
    Extensions, Method, Uri, Version,
};

pub use self::{
    builder::RequestBuilder,
    error::HttpError,
    types::{Request, RequestBody, RequestPre, Response},
};

#[cfg(feature = "async")]
pub use self::async_impl::{
    DynChunksStream, DynClient as AsyncDynClient, DynExecutor as AsyncDynExecutor,
    DynResponseBody as AsyncDynResponseBody, HttpFuture,
};

pub trait Respond: 'static {
    type Chunks;
    type BytesOutput;
    type Reader;

    fn into_chunks(self) -> Self::Chunks;
    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks;

    fn bytes(self) -> Self::BytesOutput;
    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput;

    fn reader(self) -> Self::Reader;
    fn reader_boxed(self: Box<Self>) -> Self::Reader;
}

impl<R: Respond + ?Sized> Respond for Box<R> {
    type Chunks = R::Chunks;
    type BytesOutput = R::BytesOutput;
    type Reader = R::Reader;

    fn into_chunks(self) -> Self::Chunks {
        self.into_chunks_boxed()
    }

    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks {
        R::into_chunks_boxed(*self)
    }

    fn bytes(self) -> Self::BytesOutput {
        self.bytes_boxed()
    }

    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput {
        R::bytes_boxed(*self)
    }

    fn reader(self) -> Self::Reader {
        self.reader_boxed()
    }

    fn reader_boxed(self: Box<Self>) -> Self::Reader {
        R::reader_boxed(*self)
    }
}

pub type Tapper = Arc<dyn Fn(&mut Response<()>) + Send + Sync>;

pub trait HttpExecutor {
    type RequestBody;
    type ResponseBody;
    type Output;

    fn request_body_from_generic(&self, body: RequestBody) -> Self::RequestBody;
    fn new_output_error(&self, error: HttpError) -> Self::Output;

    fn execute(&self, pre: RequestPre<Self::RequestBody>) -> Self::Output;

    fn execute_generic(&self, pre: RequestPre<RequestBody>) -> Self::Output {
        let pre2 = pre.map_body(|b| self.request_body_from_generic(b));
        self.execute(pre2)
    }
}

impl<E: HttpExecutor + ?Sized> HttpExecutor for Arc<E> {
    type RequestBody = E::RequestBody;
    type ResponseBody = E::ResponseBody;
    type Output = E::Output;

    fn request_body_from_generic(&self, body: RequestBody) -> Self::RequestBody {
        E::request_body_from_generic(self, body)
    }

    fn new_output_error(&self, error: HttpError) -> Self::Output {
        E::new_output_error(self, error)
    }

    fn execute(&self, pre: RequestPre<Self::RequestBody>) -> Self::Output {
        E::execute(self, pre)
    }
}

struct ClientInner<E> {
    exec: E,
    #[cfg(feature = "cookies")]
    cookies: Option<Arc<std::sync::RwLock<cookie_store::CookieStore>>>,
    #[allow(dead_code)]
    tapper: Option<Tapper>,
}

pub struct Client<E>(Arc<ClientInner<E>>);

impl<E> Clone for Client<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E> Client<E>
where
    E: HttpExecutor + Sized,
{
    pub fn new(exec: E) -> Self {
        Self(Arc::new(ClientInner {
            exec,
            #[cfg(feature = "cookies")]
            cookies: None,
            tapper: None,
        }))
    }

    #[cfg(feature = "cookies")]
    pub fn new_with_cookie_jar(exec: E) -> Self {
        let jar = Arc::new(std::sync::RwLock::new(cookie_store::CookieStore::default()));

        let jar2 = jar.clone();
        let tap: Tapper = Arc::new(move |res: &mut Response<()>| {
            let mut store = jar.write().unwrap();
            for header in res.headers.get_all(header::SET_COOKIE) {
                let opt = std::str::from_utf8(header.as_bytes())
                    .ok()
                    .and_then(|v| v.parse::<cookie::Cookie>().ok());

                let url_opt = res
                    .uri
                    .as_ref()
                    .and_then(|u| u.to_string().parse::<url::Url>().ok());

                if let (Some(cookie), Some(url)) = (opt, url_opt) {
                    store.store_response_cookies(Some(cookie).into_iter(), &url);
                }
            }
        });
        Self(Arc::new(ClientInner {
            exec,
            cookies: Some(jar2),
            tapper: Some(tap),
        }))
    }

    pub fn send(&self, request: Request<E::RequestBody>) -> E::Output {
        self.send_pre(RequestPre {
            request,
            timeout: None,
            tap: None,
        })
    }

    fn map_request(&self, r: Request<E::RequestBody>) -> Request<E::RequestBody> {
        #[cfg(feature = "cookies")]
        let mut r = r;
        #[cfg(feature = "cookies")]
        {
            self.0.cookies.as_ref().and_then(|jar| {
                if r.headers.contains_key(header::COOKIE) {
                    return None;
                }

                let url = r.uri.to_string().parse::<url::Url>().ok()?;
                let value = jar
                    .read()
                    .unwrap()
                    .get_request_values(&url)
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join("; ")
                    .parse::<HeaderValue>()
                    .ok()?;

                r.headers.insert(header::COOKIE, value);

                Some(())
            });
        }

        r
    }

    pub fn send_pre(&self, mut pre: RequestPre<E::RequestBody>) -> E::Output {
        pre.request = self.map_request(pre.request);
        self.0.exec.execute(pre)
    }

    pub fn request<M, U>(&self, method: M, uri: U) -> RequestBuilder<E>
    where
        Method: TryFrom<M>,
        <Method as TryFrom<M>>::Error: Into<http::Error>,
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        RequestBuilder::new(self.clone()).method(method).uri(uri)
    }

    pub fn get<U>(&self, uri: U) -> RequestBuilder<E>
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(Method::GET, uri)
    }

    pub fn head<U>(&self, uri: U) -> RequestBuilder<E>
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::HEAD, uri)
    }

    pub fn patch<U>(&self, uri: U) -> RequestBuilder<E>
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::PATCH, uri)
    }

    pub fn post<U>(&self, uri: U) -> RequestBuilder<E>
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(Method::POST, uri)
    }

    pub fn put<U>(&self, uri: U) -> RequestBuilder<E>
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(Method::PUT, uri)
    }

    pub fn delete<U>(&self, uri: U) -> RequestBuilder<E>
    where
        Uri: TryFrom<U>,
        <Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(Method::DELETE, uri)
    }
}
