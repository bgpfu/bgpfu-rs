use std::{fmt, io::Write};

use quick_xml::{events::BytesStart, NsReader, Writer};

use crate::{session::Context, Error};

use super::{Datastore, Operation, ReadXml, ReplyData, WriteXml};

#[derive(Debug, Default, Clone)]
pub struct GetConfig {
    source: Datastore,
    filter: Option<Filter>,
}

impl Operation for GetConfig {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Reply;
}

impl WriteXml for GetConfig {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("get-config")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("source")
                    .write_inner_content(|writer| self.source.write_xml(writer.get_mut()))?;
                if let Some(ref filter) = self.filter {
                    filter.write_xml(writer.get_mut())?;
                };
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Filter {
    Subtree(String),
}

impl WriteXml for Filter {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let mut writer = Writer::new(writer);
        let elem = writer.create_element("filter");
        _ = match self {
            Self::Subtree(filter) => elem
                .with_attribute(("type", "subtree"))
                .write_inner_content(|writer| {
                    writer
                        .get_mut()
                        .write_all(filter.as_bytes())
                        .map_err(|err| Error::RpcRequestSerialization(err.into()))
                })?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Builder<'a> {
    ctx: &'a Context,
    source: Option<Datastore>,
    filter: Option<Filter>,
}

impl Builder<'_> {
    #[must_use]
    pub const fn source(mut self, source: Datastore) -> Self {
        self.source = Some(source);
        self
    }

    #[must_use]
    pub fn filter(mut self, filter: Option<Filter>) -> Self {
        self.filter = filter;
        self
    }
}

impl<'a> super::Builder<'a, GetConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            source: None,
            filter: None,
        }
    }

    fn finish(self) -> Result<GetConfig, Error> {
        Ok(GetConfig {
            source: self
                .source
                .ok_or_else(|| Error::MissingOperationParameter("get-config", "source"))?,
            filter: self.filter,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    inner: Box<str>,
}

impl ReadXml for Reply {
    type Error = Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let end = start.to_end();
        let inner = reader.read_text(end.name())?.into();
        Ok(Self { inner })
    }
}

impl ReplyData for Reply {
    type Ok = Self;

    fn from_ok() -> Result<Self::Ok, Error> {
        Err(Error::EmptyRpcReply)
    }

    fn into_result(self) -> Result<Self::Ok, Error> {
        Ok(self)
    }
}

impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl AsRef<str> for Reply {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{
        rpc::{MessageId, Request},
        ClientMsg,
    };

    use quick_xml::events::Event;

    #[test]
    fn default_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: GetConfig::default(),
        };
        let expect = r#"<rpc message-id="101"><get-config><source><running/></source></get-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn reply_from_xml() {
        let reply = "<configuration><top/></configuration>";
        let expect = Reply {
            inner: reply.into(),
        };
        let msg = format!("<data>{reply}</data>");
        let mut reader = NsReader::from_str(msg.as_str());
        _ = reader.trim_text(true);
        if let Event::Start(start) = reader.read_event().unwrap() {
            assert_eq!(Reply::read_xml(&mut reader, &start).unwrap(), expect);
        } else {
            panic!("missing <data> tag")
        }
    }
}
