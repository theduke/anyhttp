use http::{Extensions, HeaderMap, HeaderValue, StatusCode, Uri, Version};

use crate::{HttpError, Respond};

pub enum RequestBody {
    Empty,
    Bytes(Vec<u8>),
    Read(Box<dyn std::io::Read>),
}

impl From<Vec<u8>> for RequestBody {
    fn from(b: Vec<u8>) -> Self {
        Self::Bytes(b)
    }
}

impl<'a> From<&'a [u8]> for RequestBody {
    fn from(b: &'a [u8]) -> Self {
        Self::Bytes(b.to_vec())
    }
}

impl<'a> From<&'a str> for RequestBody {
    fn from(b: &'a str) -> Self {
        Self::Bytes(b.as_bytes().to_vec())
    }
}

impl From<String> for RequestBody {
    fn from(b: String) -> Self {
        Self::Bytes(b.into_bytes())
    }
}

pub struct Response<B> {
    parts: http::response::Parts,
    uri: http::Uri,
    body: B,
}

impl<B> Response<B> {
    /// Creates a new blank `Response` with the body
    ///
    /// The component ports of this response will be set to their default, e.g.
    /// the ok status, no headers, etc.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response = Response::new("hello world");
    ///
    /// assert_eq!(response.status(), StatusCode::OK);
    /// assert_eq!(*response.body(), "hello world");
    /// ```
    #[inline]
    pub fn new(body: B) -> Self {
        let parts = http::response::Builder::new()
            .body(())
            .unwrap()
            .into_parts()
            .0;
        Self {
            parts,
            body,
            uri: http::Uri::default(),
        }
    }

    /// Creates a new `Response` with the given head and body
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response = Response::new("hello world");
    /// let (mut parts, body) = response.into_parts();
    ///
    /// parts.status = StatusCode::BAD_REQUEST;
    /// let response = Response::from_parts(parts, body);
    ///
    /// assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    /// assert_eq!(*response.body(), "hello world");
    /// ```
    #[inline]
    pub fn from_parts(parts: http::response::Parts, body: B) -> Self {
        Self {
            parts,
            body,
            uri: Uri::default(),
        }
    }

    /// Returns the `StatusCode`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response: Response<()> = Response::default();
    /// assert_eq!(response.status(), StatusCode::OK);
    /// ```
    #[inline]
    pub fn status(&self) -> StatusCode {
        self.parts.status
    }

    /// Returns a mutable reference to the associated `StatusCode`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let mut response: Response<()> = Response::default();
    /// *response.status_mut() = StatusCode::CREATED;
    /// assert_eq!(response.status(), StatusCode::CREATED);
    /// ```
    #[inline]
    pub fn status_mut(&mut self) -> &mut StatusCode {
        &mut self.parts.status
    }

    /// Returns a reference to the associated version.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response: Response<()> = Response::default();
    /// assert_eq!(response.version(), Version::HTTP_11);
    /// ```
    #[inline]
    pub fn version(&self) -> Version {
        self.parts.version
    }

    /// Returns a mutable reference to the associated version.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let mut response: Response<()> = Response::default();
    /// *response.version_mut() = Version::HTTP_2;
    /// assert_eq!(response.version(), Version::HTTP_2);
    /// ```
    #[inline]
    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.parts.version
    }

    #[inline]
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    #[inline]
    pub fn uri_mut(&mut self) -> &mut Uri {
        &mut self.uri
    }

    /// Returns a reference to the associated header field map.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response: Response<()> = Response::default();
    /// assert!(response.headers().is_empty());
    /// ```
    #[inline]
    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        &self.parts.headers
    }

    /// Returns a mutable reference to the associated header field map.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// # use http::header::*;
    /// let mut response: Response<()> = Response::default();
    /// response.headers_mut().insert(HOST, HeaderValue::from_static("world"));
    /// assert!(!response.headers().is_empty());
    /// ```
    #[inline]
    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        &mut self.parts.headers
    }

    /// Returns a reference to the associated extensions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response: Response<()> = Response::default();
    /// assert!(response.extensions().get::<i32>().is_none());
    /// ```
    #[inline]
    pub fn extensions(&self) -> &Extensions {
        &self.parts.extensions
    }

    /// Returns a mutable reference to the associated extensions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// # use http::header::*;
    /// let mut response: Response<()> = Response::default();
    /// response.extensions_mut().insert("hello");
    /// assert_eq!(response.extensions().get(), Some(&"hello"));
    /// ```
    #[inline]
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.parts.extensions
    }

    /// Returns a reference to the associated HTTP body.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response: Response<String> = Response::default();
    /// assert!(response.body().is_empty());
    /// ```
    #[inline]
    pub fn body(&self) -> &B {
        &self.body
    }

    /// Returns a mutable reference to the associated HTTP body.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let mut response: Response<String> = Response::default();
    /// response.body_mut().push_str("hello world");
    /// assert!(!response.body().is_empty());
    /// ```
    #[inline]
    pub fn body_mut(&mut self) -> &mut B {
        &mut self.body
    }

    /// Consumes the response, returning just the body.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::Response;
    /// let response = Response::new(10);
    /// let body = response.into_body();
    /// assert_eq!(body, 10);
    /// ```
    #[inline]
    pub fn into_body(self) -> B {
        self.body
    }

    /// Consumes the response returning the head and body parts.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response: Response<()> = Response::default();
    /// let (parts, body) = response.into_parts();
    /// assert_eq!(parts.status, StatusCode::OK);
    /// ```
    #[inline]
    pub fn into_parts(self) -> (http::response::Parts, B) {
        (self.parts, self.body)
    }

    /// Consumes the response returning a new response with body mapped to the
    /// return type of the passed in function.
    ///
    /// # Examples
    ///
    /// ```
    /// # use http::*;
    /// let response = Response::builder().body("some string").unwrap();
    /// let mapped_response: Response<&[u8]> = response.map(|b| {
    ///   assert_eq!(b, "some string");
    ///   b.as_bytes()
    /// });
    /// assert_eq!(mapped_response.body(), &"some string".as_bytes());
    /// ```
    #[inline]
    pub fn map<F, U>(self, f: F) -> Response<U>
    where
        F: FnOnce(B) -> U,
    {
        Response {
            body: f(self.body),
            parts: self.parts,
            uri: self.uri,
        }
    }

    pub fn take_body(self) -> (Response<()>, B) {
        (
            Response {
                parts: self.parts,
                uri: self.uri,
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
        if !self.status().is_success() {
            Err(HttpError::new(
                crate::error::Kind::NonSuccessStatus(self.status()),
                None,
                None,
            ))
        } else {
            Ok(self)
        }
    }
}

impl<B: std::fmt::Debug> std::fmt::Debug for Response<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Response")
            .field("status", &self.parts.status)
            .field("headers", &self.parts.headers)
            .field("version", &self.parts.version)
            .field("body", &self.body)
            .finish()
    }
}

impl<B> From<http::Response<B>> for Response<B> {
    fn from(r: http::Response<B>) -> Self {
        let (parts, body) = r.into_parts();
        Self {
            parts,
            body,
            uri: Uri::default(),
        }
    }
}

impl<B> From<Response<B>> for http::Response<B> {
    fn from(r: Response<B>) -> Self {
        http::Response::from_parts(r.parts, r.body)
    }
}
