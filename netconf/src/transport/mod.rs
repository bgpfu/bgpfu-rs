use async_trait::async_trait;
use bytes::Bytes;

use crate::Error;

#[cfg(feature = "ssh")]
mod ssh;
#[cfg(feature = "ssh")]
pub use self::ssh::{Password, Ssh};

#[cfg(feature = "tls")]
mod tls;
#[cfg(feature = "tls")]
pub use self::tls::Tls;

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
