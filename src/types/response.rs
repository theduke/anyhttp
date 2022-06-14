use http::{Extensions, HeaderMap, HeaderValue, StatusCode, Version};

use crate::{HttpError, Respond};

pub struct Response<B> {
    /// The final URI of of the response.
    pub uri: Option<http::Uri>,

    /// The HTTP status.
    pub status: StatusCode,

    /// The HTTP protocol version.
    pub version: Version,

    /// The response headers.
    pub headers: HeaderMap<HeaderValue>,

    /// Extensions that carry additional data.
    pub extensions: Extensions,

    pub body: B,
}

impl<B: Default> Default for Response<B> {
    fn default() -> Self {
        Self {
            uri: Default::default(),
            status: Default::default(),
            version: Default::default(),
            headers: Default::default(),
            extensions: Default::default(),
            body: Default::default(),
        }
    }
}

impl<B: Clone> Clone for Response<B> {
    fn clone(&self) -> Self {
        Self {
            uri: self.uri.clone(),
            status: self.status.clone(),
            version: self.version.clone(),
            headers: self.headers.clone(),
            extensions: Default::default(),
            body: self.body.clone(),
        }
    }
}

impl<B: std::fmt::Debug> std::fmt::Debug for Response<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("version", &self.version)
            .field("body", &self.body)
            .finish()
    }
}

impl<B> From<http::Response<B>> for Response<B> {
    fn from(r: http::Response<B>) -> Self {
        let (parts, body) = r.into_parts();
        Self {
            uri: None,
            status: parts.status,
            version: parts.version,
            headers: parts.headers,
            extensions: parts.extensions,
            body,
        }
    }
}

impl<B> From<Response<B>> for http::Response<B> {
    fn from(r: Response<B>) -> Self {
        let mut res2 = http::Response::new(r.body);
        *res2.status_mut() = r.status;
        *res2.version_mut() = r.version;
        *res2.headers_mut() = r.headers;
        *res2.extensions_mut() = r.extensions;

        res2
    }
}

impl<B> Response<B> {
    pub fn new(body: B) -> Self {
        Self {
            uri: None,
            body,
            status: Default::default(),
            version: Default::default(),
            headers: Default::default(),
            extensions: Default::default(),
        }
    }

    pub fn map_body<F, U>(self, f: F) -> Response<U>
    where
        F: FnOnce(B) -> U,
    {
        Response {
            uri: self.uri,
            status: self.status,
            version: self.version,
            headers: self.headers,
            extensions: self.extensions,
            body: f(self.body),
        }
    }

    pub fn take_body(self) -> (Response<()>, B) {
        (
            Response {
                uri: self.uri,
                status: self.status,
                version: self.version,
                headers: self.headers,
                extensions: self.extensions,
                body: (),
            },
            self.body,
        )
    }

    pub fn bytes(self) -> B::BytesOutput
    where
        B: Respond,
    {
        self.body.bytes()
    }

    pub fn error_for_status(self) -> Result<Self, HttpError> {
        if !self.status.is_success() {
            Err(HttpError::new(
                crate::error::Kind::NonSuccessStatus(self.status),
                None,
                None,
            ))
        } else {
            Ok(self)
        }
    }
}
