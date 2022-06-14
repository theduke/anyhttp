use std::str::FromStr;

use anyhttp::{sync::GenericResponseBody, HttpError, HttpExecutor};
use http::HeaderValue;

#[derive(Clone)]
pub struct UreqExecutor {
    agent: ureq::Agent,
}

impl UreqExecutor {
    pub fn new() -> Self {
        Self {
            agent: ureq::agent(),
        }
    }
}

impl HttpExecutor for UreqExecutor {
    type RequestBody = anyhttp::RequestBody;
    type ResponseBody = GenericResponseBody;

    type Output = Result<anyhttp::Response<GenericResponseBody>, anyhttp::HttpError>;

    fn request_body_from_generic(&self, body: anyhttp::RequestBody) -> Self::RequestBody {
        body
    }

    fn new_output_error(&self, error: anyhttp::HttpError) -> Self::Output {
        Err(error)
    }

    fn execute(&self, pre: anyhttp::RequestPre<Self::RequestBody>) -> Self::Output {
        let req = pre.request;
        let tap = pre.tap;

        let mut ur = self
            .agent
            .request(req.method.as_str(), &req.uri.to_string());

        for key in req.headers.keys() {
            for value in req.headers.get_all(key) {
                let value_str = std::str::from_utf8(value.as_bytes()).map_err(|_err| {
                    HttpError::new_custom(
                        "could not re-parse request header '{key}': non-utf8 value",
                    )
                })?;
                ur = ur.set(key.as_str(), value_str);
            }
        }

        let result = match req.body {
            anyhttp::RequestBody::Empty => ur.call(),
            anyhttp::RequestBody::Bytes(bytes) => ur.send_bytes(&bytes),
            anyhttp::RequestBody::Read(r) => ur.send(r),
        };

        let ures = match result {
            Ok(r) => r,
            Err(ureq::Error::Status(_status, res)) => res,
            Err(err) => {
                // FIXME: better mapping
                return Err(HttpError::new_custom(err.to_string()));
            }
        };

        let uri = ures
            .get_url()
            .parse::<http::Uri>()
            .map_err(|err| HttpError::new_http(err.into()))?;

        let status = http::StatusCode::from_u16(ures.status())
            .map_err(|err| HttpError::new_http(err.into()))?;

        let mut headers = http::HeaderMap::new();
        for header in ures.headers_names() {
            if let Some(value_raw) = ures.header(&header) {
                let key = http::header::HeaderName::from_str(&header)
                    .map_err(|err| HttpError::new_http(err.into()))?;

                let value = value_raw
                    .parse::<HeaderValue>()
                    .map_err(|err| HttpError::new_http(err.into()))?;
                headers.append(key, value);
            }
        }

        let body = GenericResponseBody::Read(Box::new(ures.into_reader()));

        let mut res = anyhttp::Response {
            uri: Some(uri),
            status,
            version: http::Version::HTTP_11,
            headers,
            extensions: Default::default(),
            body: (),
        };
        if let Some(tap) = tap {
            tap(&mut res)
        }

        let final_res = res.map_body(|_| body);
        Ok(final_res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ureq_client() {
        let exec = UreqExecutor::new();
        anyhttp::test::test_sync_executor(exec);
    }
}
