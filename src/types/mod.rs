mod request;
mod response;

use crate::Tapper;

pub use self::{request::Request, response::Response};

pub enum RequestBody {
    Empty,
    Bytes(Vec<u8>),
    Read(Box<dyn std::io::Read>),
}

impl std::fmt::Debug for RequestBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Empty"),
            Self::Bytes(arg0) => f.debug_tuple("Bytes").field(arg0).finish(),
            Self::Read(_) => f.debug_tuple("Read").field(&"...").finish(),
        }
    }
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

pub struct RequestPre<B> {
    pub request: Request<B>,
    pub timeout: Option<std::time::Duration>,
    pub tap: Option<Tapper>,
}

impl<B> RequestPre<B> {
    pub fn map_body<B2, F: FnOnce(B) -> B2>(self, f: F) -> RequestPre<B2> {
        RequestPre {
            request: self.request.map_body(f),
            timeout: self.timeout,
            tap: self.tap,
        }
    }
}
