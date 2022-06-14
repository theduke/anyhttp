use http::{header::HeaderName, HeaderValue, Method, Uri};

use crate::{Client, HttpError, HttpExecutor, Request, RequestBody, RequestPre};

pub struct RequestBuilder<E: HttpExecutor> {
    client: Client<E>,
    result: Result<RequestPre<E::RequestBody>, HttpError>,
}

impl<E: HttpExecutor + Sized> RequestBuilder<E> {
    pub fn new(client: Client<E>) -> Self {
        let body = client.0.exec.request_body_from_generic(RequestBody::Empty);
        let pre = RequestPre {
            request: Request::new(body),
            timeout: None,
            tap: None,
        };
        Self {
            client,
            result: Ok(pre),
        }
    }

    pub fn version(mut self, version: http::Version) -> Self {
        self.result = self.result.and_then(|mut pre| {
            pre.request.version = version;
            Ok(pre)
        });
        self
    }

    pub fn method<T>(mut self, method: T) -> Self
    where
        Method: TryFrom<T>,
        <Method as TryFrom<T>>::Error: Into<http::Error>,
    {
        self.result = self.result.and_then(move |mut r| {
            let method = Method::try_from(method)
                .map_err(|err| HttpError::new_invalid_request(err.into(), None))?;
            r.request.method = method;
            Ok(r)
        });
        self
    }

    pub fn uri<T>(mut self, uri: T) -> Self
    where
        http::Uri: TryFrom<T>,
        <http::Uri as TryFrom<T>>::Error: Into<http::Error>,
    {
        self.result = self.result.and_then(move |mut r| {
            let uri = Uri::try_from(uri)
                .map_err(|err| HttpError::new_invalid_request(err.into(), None))?;
            r.request.uri = uri;
            Ok(r)
        });
        self
    }

    pub fn uri_mut(&mut self) -> Option<&mut Uri> {
        self.result.as_mut().ok().map(|p| &mut p.request.uri)
    }

    pub fn header_sensitive<K, V>(mut self, key: K, value: V, is_sensitive: bool) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.result = self.result.and_then(move |mut r| {
            let key = HeaderName::try_from(key)
                .map_err(|err| HttpError::new_invalid_request(err.into(), None))?;
            let mut value = HeaderValue::try_from(value)
                .map_err(|err| HttpError::new_invalid_request(err.into(), None))?;
            value.set_sensitive(is_sensitive);
            r.request.headers.append(key, value);

            Ok(r)
        });
        self
    }

    pub fn header<K, V>(self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
    {
        self.header_sensitive(key, value, false)
    }

    pub fn headers(mut self, headers: http::HeaderMap) -> Self {
        self.result = self.result.and_then(|mut pre| {
            pre.request.headers.extend(headers);
            Ok(pre)
        });
        self
    }

    #[cfg(feature = "base64")]
    pub fn basic_auth<U, P>(self, username: U, password: Option<P>) -> Self
    where
        U: std::fmt::Display,
        P: std::fmt::Display,
    {
        use std::io::Write;

        let mut header_value = b"Basic ".to_vec();
        {
            let mut encoder =
                base64::write::EncoderWriter::new(&mut header_value, base64::STANDARD);
            // The unwraps here are fine because Vec::write* is infallible.
            write!(encoder, "{}:", username).unwrap();
            if let Some(password) = password {
                write!(encoder, "{}", password).unwrap();
            }
        }

        self.header_sensitive(http::header::AUTHORIZATION, header_value, true)
    }

    pub fn bearer_auth<T>(self, token: T) -> Self
    where
        T: std::fmt::Display,
    {
        let header_value = format!("Bearer {}", token);
        self.header_sensitive(http::header::AUTHORIZATION, header_value, true)
    }

    pub fn body<B>(mut self, body: B) -> Self
    where
        E::RequestBody: TryFrom<B>,
        <E::RequestBody as TryFrom<B>>::Error: std::error::Error + Send + Sync + 'static,
    {
        self.result = self.result.and_then(move |mut r| {
            let body = E::RequestBody::try_from(body)
                .map_err(|err| HttpError::new_invalid_request(err, None))?;
            r.request.body = body;
            Ok(r)
        });
        self
    }

    #[cfg(feature = "json")]
    pub fn json<T: serde::Serialize + ?Sized>(mut self, value: &T) -> Self {
        let client = &self.client;
        self.result = self.result.and_then(move |mut r| {
            let raw_body = serde_json::to_vec(value).map_err(|err| {
                HttpError::new(
                    crate::error::Kind::InvalidRequestJson,
                    Some(Box::new(err)),
                    None,
                )
            })?;
            let body = client
                .0
                .exec
                .request_body_from_generic(RequestBody::Bytes(raw_body));

            r.request.body = body;

            r.request.headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            Ok(r)
        });
        self
    }

    #[cfg(feature = "urlencoding")]
    pub fn form<T: serde::Serialize>(mut self, data: &T) -> Self {
        self.result = self.result.and_then(|mut pre| {
            let body = serde_urlencoded::to_string(data)
                .map(|s| {
                    self.client
                        .0
                        .exec
                        .request_body_from_generic(RequestBody::Bytes(s.into_bytes()))
                })
                .map_err(|err| {
                    HttpError::new(
                        crate::error::Kind::InvalidRequestJson,
                        Some(Box::new(err)),
                        None,
                    )
                })?;
            pre.request.body = body;
            pre.request.headers.insert(
                http::header::CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            );

            Ok(pre)
        });
        self
    }

    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.result = self.result.and_then(move |mut pre| {
            pre.timeout = Some(timeout);
            Ok(pre)
        });
        self
    }

    pub fn build(self) -> Result<RequestPre<E::RequestBody>, HttpError> {
        self.result
    }

    pub fn send(self) -> <E as HttpExecutor>::Output {
        match self.result {
            Ok(pre) => self.client.send_pre(pre),
            Err(err) => self.client.0.exec.new_output_error(err),
        }
    }
}
