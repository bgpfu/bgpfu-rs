use std::{fmt::Debug, io::Write, str::from_utf8, string::FromUtf8Error};

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

mod hello;
pub(crate) use self::hello::{Capabilities, Capability, ClientHello, ServerHello, BASE};

pub mod rpc;

mod xmlns;

pub(crate) const MARKER: &[u8] = b"]]>]]>";

pub trait ReadXml: Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error>;
}

pub trait WriteXml {
    type Error: From<FromUtf8Error> + std::error::Error + Send + Sync + 'static;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error>;
}

#[async_trait]
pub trait ClientMsg: WriteXml + Debug
where
    Error: From<Self::Error>,
{
    fn to_xml(&self) -> Result<String, Self::Error> {
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
    fn from_xml<S>(input: S) -> Result<Self, Error>
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
                    this = Some(
                        Self::read_xml(&mut reader, &tag)
                            .map_err(|err| Error::ReadXml(err.into()))?,
                    );
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::Eof) => break,
                (_, Event::Text(txt)) if &*txt == MARKER => break,
                // TODO:
                // We should save the namespace in the error too.
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        this.ok_or_else(|| Error::MissingElement(Self::TAG_NAME, Self::TAG_NAME))
    }

    #[tracing::instrument(skip(receiver), err)]
    async fn recv<T: RecvHandle>(receiver: &mut T) -> Result<Self, Error> {
        let bytes = receiver.recv().await?;
        let serialized = from_utf8(&bytes)?;
        Ok(Self::from_xml(serialized)?)
    }
}
