use std::{
    convert::Infallible,
    fmt::{self, Debug},
    str::FromStr,
};

use async_trait::async_trait;
use bytes::Bytes;

use crate::Error;

mod ssh;
pub use self::ssh::Ssh;

pub trait Transport: Send {
    type SendHandle: SendHandle;
    type RecvHandle: RecvHandle;

    fn split(self) -> (Self::SendHandle, Self::RecvHandle);
}

#[async_trait]
pub trait SendHandle: Send {
    async fn send(&mut self, data: Bytes) -> Result<(), Error>;
}

#[async_trait]
pub trait RecvHandle: Send {
    async fn recv(&mut self) -> Result<Bytes, Error>;
}

#[derive(Clone)]
pub struct Password(String);

impl Password {
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Password").field(&"****").finish()
    }
}

impl FromStr for Password {
    type Err = Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}
