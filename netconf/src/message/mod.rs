use std::{fmt::Debug, io::Write, str::from_utf8};

use async_trait::async_trait;
use quick_xml::{
    events::{BytesStart, Event},
    name::{Namespace, ResolveResult},
    NsReader,
};

use crate::{
    transport::{RecvHandle, SendHandle},
    Error,
};

mod error;
pub use self::error::{Read as ReadError, Write as WriteError};

mod hello;
pub(crate) use self::hello::{ClientHello, ServerHello};

pub mod rpc;

pub(crate) mod xmlns;

pub(crate) const MARKER: &[u8] = b"]]>]]>";

pub trait ReadXml: Sized {
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError>;
}

pub trait WriteXml {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError>;
}

#[async_trait]
pub trait ClientMsg: WriteXml + Debug {
    fn to_xml(&self) -> Result<String, WriteError> {
        let mut buf = Vec::new();
        self.write_xml(&mut buf)?;
        buf.extend_from_slice(MARKER);
        Ok(String::from_utf8(buf)?)
    }

    #[tracing::instrument(skip(sender), err, level = "debug")]
    async fn send<T: SendHandle>(&self, sender: &mut T) -> Result<(), Error> {
        let serialized = self.to_xml()?;
        sender.send(serialized.into()).await
    }
}

#[async_trait]
pub trait ServerMsg: ReadXml {
    const TAG_NS: Namespace<'static>;
    const TAG_NAME: &'static str;

    #[tracing::instrument(skip(input))]
    fn from_xml<S>(input: S) -> Result<Self, ReadError>
    where
        S: AsRef<str> + Debug,
    {
        tracing::debug!(?input);
        let mut reader = NsReader::from_str(input.as_ref());
        _ = reader.trim_text(true);
        tracing::debug!("expecting <{}>", Self::TAG_NAME);
        let mut this = None;
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == Self::TAG_NS
                        && tag.local_name().as_ref() == Self::TAG_NAME.as_bytes() =>
                {
                    this = Some(Self::read_xml(&mut reader, &tag)?);
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::Eof) => break,
                (_, Event::Text(txt)) if &*txt == MARKER => break,
                // TODO:
                // We should save the namespace in the error too.
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        this.ok_or_else(|| ReadError::missing_element(Self::TAG_NAME, Self::TAG_NAME))
    }

    #[tracing::instrument(skip(receiver), err)]
    async fn recv<T: RecvHandle>(receiver: &mut T) -> Result<Self, Error> {
        let bytes = receiver.recv().await?;
        let serialized = from_utf8(&bytes).map_err(ReadError::DecodeMessage)?;
        Ok(Self::from_xml(serialized)?)
    }
}
