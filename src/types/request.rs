use crate::{header::HeaderMap, Extensions, Method, Uri, Version};

#[derive(Debug)]
pub struct Request<B> {
    pub method: Method,
    pub uri: Uri,
    pub version: Version,
    pub headers: HeaderMap,
    pub extensions: Extensions,
    pub body: B,
}

impl<B> Request<B> {
    pub fn new(body: B) -> Self {
        Self {
            method: Default::default(),
            uri: Default::default(),
            version: Default::default(),
            headers: Default::default(),
            extensions: Default::default(),
            body,
        }
    }
}

impl<B> From<http::Request<B>> for Request<B> {
    fn from(r: http::Request<B>) -> Self {
        let (parts, body) = r.into_parts();

        Self {
            method: parts.method,
            uri: parts.uri,
            version: parts.version,
            headers: parts.headers,
            extensions: parts.extensions,
            body,
        }
    }
}

impl<B> From<Request<B>> for http::Request<B> {
    fn from(r: Request<B>) -> Self {
        let mut req2 = http::Request::new(r.body);
        *req2.method_mut() = r.method;
        *req2.uri_mut() = r.uri;
        *req2.version_mut() = r.version;
        *req2.headers_mut() = r.headers;
        *req2.extensions_mut() = r.extensions;
        req2
    }
}

impl<B> Request<B> {
    pub fn map_body<B2, F>(self, f: F) -> Request<B2>
    where
        F: FnOnce(B) -> B2,
    {
        Request {
            method: self.method,
            uri: self.uri,
            version: self.version,
            headers: self.headers,
            extensions: self.extensions,
            body: f(self.body),
        }
    }
}
