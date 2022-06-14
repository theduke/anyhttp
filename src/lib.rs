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

pub use self::{
    builder::RequestBuilder,
    error::HttpError,
    types::{RequestBody, Response},
};

#[cfg(feature = "async")]
pub use self::async_impl::{
    DynChunksStream, DynClient as AsyncDynClient, DynExecutor as AsyncDynExecutor,
    DynResponseBody as AsyncDynResponseBody, HttpFuture,
};

pub trait Respond: 'static {
    type Chunks;
    type BytesOutput;

    fn into_chunks(self) -> Self::Chunks;
    fn into_chunks_boxed(self: Box<Self>) -> Self::Chunks;

    fn bytes(self) -> Self::BytesOutput;
    fn bytes_boxed(self: Box<Self>) -> Self::BytesOutput;
}

impl<R: Respond + ?Sized> Respond for Box<R> {
    type Chunks = R::Chunks;

    type BytesOutput = R::BytesOutput;

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
        let (parts, body) = pre.request.into_parts();
        let body2 = self.request_body_from_generic(body);
        let pre2 = RequestPre {
            request: http::Request::from_parts(parts, body2),
            timeout: pre.timeout,
            tap: pre.tap,
        };
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

pub struct RequestPre<Body> {
    pub request: http::Request<Body>,
    pub timeout: Option<std::time::Duration>,
    pub tap: Option<Tapper>,
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
            for header in res.headers().get_all(http::header::SET_COOKIE) {
                let opt = std::str::from_utf8(header.as_bytes())
                    .map(|x| x.to_string())
                    .map_err(cookie::ParseError::from)
                    .and_then(cookie::Cookie::parse);

                let url = res.uri().to_string().parse::<url::Url>();

                if let (Ok(cookie), Ok(url)) = (opt, url) {
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

    pub fn send(&self, request: http::Request<E::RequestBody>) -> E::Output {
        self.send_pre(RequestPre {
            request,
            timeout: None,
            tap: None,
        })
    }

    fn map_request(&self, r: http::Request<E::RequestBody>) -> http::Request<E::RequestBody> {
        #[cfg(feature = "cookies")]
        let mut r = r;
        #[cfg(feature = "cookies")]
        {
            self.0.cookies.as_ref().and_then(|jar| {
                if r.headers().contains_key(http::header::COOKIE) {
                    return None;
                }

                let url = r.uri().to_string().parse::<url::Url>().ok()?;
                let value = jar
                    .read()
                    .unwrap()
                    .get_request_values(&url)
                    .map(|(name, value)| format!("{name}={value}"))
                    .collect::<Vec<_>>()
                    .join("; ")
                    .parse::<http::HeaderValue>()
                    .ok()?;

                r.headers_mut().insert(http::header::COOKIE, value);

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
        http::Method: TryFrom<M>,
        <http::Method as TryFrom<M>>::Error: Into<http::Error>,
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        RequestBuilder::new(self.clone()).method(method).uri(uri)
    }

    pub fn get<U>(&self, uri: U) -> RequestBuilder<E>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::GET, uri)
    }

    pub fn head<U>(&self, uri: U) -> RequestBuilder<E>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::HEAD, uri)
    }

    pub fn patch<U>(&self, uri: U) -> RequestBuilder<E>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::PATCH, uri)
    }

    pub fn post<U>(&self, uri: U) -> RequestBuilder<E>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::POST, uri)
    }

    pub fn put<U>(&self, uri: U) -> RequestBuilder<E>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::PUT, uri)
    }

    pub fn delete<U>(&self, uri: U) -> RequestBuilder<E>
    where
        http::Uri: TryFrom<U>,
        <http::Uri as TryFrom<U>>::Error: Into<http::Error>,
    {
        self.request(http::Method::DELETE, uri)
    }
}
