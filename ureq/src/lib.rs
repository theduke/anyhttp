use std::str::FromStr;

use anyhttp::{sync::GenericResponseBody, HttpError, HttpExecutor};

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
        let (parts, body) = pre.request.into_parts();

        let mut ur = self
            .agent
            .request(parts.method.as_str(), &parts.uri.to_string());

        for key in parts.headers.keys() {
            for value in parts.headers.get_all(key) {
                let value_str = std::str::from_utf8(value.as_bytes()).map_err(|_err| {
                    HttpError::new_custom(
                        "could not re-parse request header '{key}': non-utf8 value",
                    )
                })?;
                ur = ur.set(key.as_str(), value_str);
            }
        }

        let res = match body {
            anyhttp::RequestBody::Empty => ur.call(),
            anyhttp::RequestBody::Bytes(bytes) => ur.send_bytes(&bytes),
            anyhttp::RequestBody::Read(r) => ur.send(r),
        };

        match res {
            Ok(res) => {
                let uri = res
                    .get_url()
                    .parse::<http::Uri>()
                    .map_err(|err| HttpError::new_http(err.into()))?;

                let mut builder = http::response::Response::builder().status(res.status());
                for header in res.headers_names() {
                    if let Some(value) = res.header(&header) {
                        builder = builder.header(&header, value);
                    }
                }

                let (parts, _) = builder.body(()).unwrap().into_parts();
                let body = GenericResponseBody::Read(Box::new(res.into_reader()));
                let mut fres = anyhttp::Response::from_parts(parts, body);
                *fres.uri_mut() = uri;
                Ok(fres)
            }
            Err(err) => {
                // FIXME: better mapping
                Err(HttpError::new_custom(err.to_string()))
            }
        }
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
