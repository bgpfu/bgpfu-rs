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

    fn split(&mut self) -> (&mut Self::SendHandle, &mut Self::RecvHandle);
}

#[async_trait]
pub trait SendHandle: Send {
    async fn send(&mut self, data: Bytes) -> Result<(), Error>;
}

#[async_trait]
impl<T: Transport> SendHandle for T {
    async fn send(&mut self, data: Bytes) -> Result<(), Error> {
        let (tx, _) = self.split();
        tx.send(data).await
    }
}

#[async_trait]
pub trait RecvHandle: Send {
    async fn recv(&mut self) -> Result<Bytes, Error>;
}

#[async_trait]
impl<T: Transport> RecvHandle for T {
    async fn recv(&mut self) -> Result<Bytes, Error> {
        let (_, rx) = self.split();
        rx.recv().await
    }
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
