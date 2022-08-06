use http::StatusCode;

#[derive(Debug)]
pub struct HttpError {
    kind: Kind,
    cause: Option<DynError>,
    message: Option<String>,
}

type DynError = Box<dyn std::error::Error + Send + Sync>;

impl HttpError {
    pub(crate) fn new(kind: Kind, cause: Option<DynError>, message: Option<String>) -> Self {
        Self {
            kind,
            cause,
            message,
        }
    }

    pub fn new_invalid_request(
        error: impl std::error::Error + Send + Sync + 'static,
        message: Option<String>,
    ) -> Self {
        Self {
            kind: Kind::InvalidRequest,
            cause: Some(Box::new(error)),
            message,
        }
    }

    pub fn new_io(error: std::io::Error, message: Option<String>) -> Self {
        Self {
            kind: Kind::Io,
            cause: Some(Box::new(error)),
            message,
        }
    }

    pub fn new_http(error: http::Error) -> Self {
        Self {
            kind: Kind::Http,
            cause: Some(Box::new(error)),
            message: None,
        }
    }

    pub fn new_response_read<E>(message: Option<String>, error: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind: Kind::ResponseRead,
            cause: Some(Box::new(error)),
            message,
        }
    }

    pub fn is_invalid_request(&self) -> bool {
        matches!(self.kind, Kind::InvalidRequest) || matches!(self.kind, Kind::InvalidRequest)
    }

    pub fn new_custom(message: impl Into<String>) -> Self {
        Self {
            kind: Kind::Other,
            cause: None,
            message: Some(message.into()),
        }
    }

    pub fn new_custom_with_cause<E>(message: impl Into<String>, cause: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self {
            kind: Kind::Other,
            cause: Some(Box::new(cause)),
            message: Some(message.into()),
        }
    }

    pub fn as_status(&self) -> Option<StatusCode> {
        match self.kind {
            Kind::NonSuccessStatus(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_not_found(&self) -> bool {
        if let Some(s) = self.as_status() {
            s == StatusCode::NOT_FOUND
        } else {
            false
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.kind {
            Kind::InvalidRequest => {
                write!(f, "invalid request")?;
                true
            }
            #[cfg(feature = "json")]
            Kind::InvalidRequestJson => {
                write!(f, "invalid request json")?;
                true
            }
            Kind::NonSuccessStatus(status) => {
                write!(
                    f,
                    "Request failed with status {}({})",
                    status.as_str(),
                    status.as_u16()
                )?;
                true
            }
            Kind::ResponseRead => {
                write!(f, "could not read response body")?;
                true
            }
            Kind::Http => false,
            Kind::Other => false,
            #[cfg(feature = "json")]
            Kind::InvalidResponseJson => {
                write!(f, "could not deserialize JSON response")?;
                true
            }
            Kind::Io => {
                write!(f, "io error")?;
                true
            }
        };

        let prefix = if let Some(msg) = &self.message {
            if prefix {
                write!(f, ": ")?;
            }
            write!(f, "{}", msg)?;
            true
        } else {
            prefix
        };

        if let Some(cause) = &self.cause {
            if prefix {
                write!(f, ": ")?;
            }
            write!(f, "{}", cause)?;
        }

        Ok(())
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(e) = self.cause.as_ref() {
            Some(&**e)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub(crate) enum Kind {
    InvalidRequest,
    #[cfg(feature = "json")]
    InvalidRequestJson,
    #[cfg(feature = "json")]
    InvalidResponseJson,
    NonSuccessStatus(http::StatusCode),
    ResponseRead,
    Http,
    Io,
    Other,
}
