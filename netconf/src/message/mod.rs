use std::{fmt::Debug, io::Write, str::from_utf8, string::FromUtf8Error};

use async_trait::async_trait;

use crate::{
    transport::{RecvHandle, SendHandle},
    Error,
};

mod hello;
pub(crate) use self::hello::{Capabilities, Capability, ClientHello, ServerHello, BASE};

pub mod rpc;

const MARKER: &[u8] = b"]]>]]>";

pub trait FromXml: Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn from_xml<S>(input: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug;
}

pub trait ToXml {
    type Error: From<FromUtf8Error> + std::error::Error + Send + Sync + 'static;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error>;

    fn to_xml(&self) -> Result<String, Self::Error> {
        let mut buf = Vec::new();
        self.write_xml(&mut buf)?;
        Ok(String::from_utf8(buf)?)
    }
}

#[async_trait]
pub trait ClientMsg: ToXml + Debug
where
    Error: From<Self::Error>,
{
    #[tracing::instrument(skip(sender))]
    async fn send<T: SendHandle>(&self, sender: &mut T) -> Result<(), Error> {
        let serialized = self.to_xml()?;
        sender.send(serialized.into()).await
    }
}

#[async_trait]
pub trait ServerMsg: FromXml
where
    Error: From<Self::Error>,
{
    #[tracing::instrument(skip(receiver))]
    async fn recv<T: RecvHandle>(receiver: &mut T) -> Result<Self, Error> {
        let bytes = receiver.recv().await?;
        let serialized = from_utf8(&bytes)?;
        Ok(Self::from_xml(serialized)?)
    }
}
