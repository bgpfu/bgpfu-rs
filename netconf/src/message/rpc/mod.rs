use std::{
    convert::Infallible,
    fmt::{self, Debug},
};

use async_trait::async_trait;
use quick_xml::{
    events::{attributes::Attribute, Event},
    Reader,
};
use serde::{Deserialize, Serialize};

use super::{ClientMsg, FromXml, ServerMsg, ToXml};

pub mod operation;
pub use self::operation::Operation;

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(usize);

impl MessageId {
    pub(crate) fn increment(&mut self) -> Self {
        self.0 += 1;
        *self
    }
}

impl TryFrom<Attribute<'_>> for MessageId {
    type Error = crate::Error;

    fn try_from(value: Attribute<'_>) -> Result<Self, Self::Error> {
        Ok(Self(value.unescape_value()?.as_ref().parse()?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Request<O: Operation> {
    #[serde(rename = "@message-id")]
    message_id: MessageId,
    #[serde(flatten)]
    operation: O::RequestData,
}

impl<O: Operation> Request<O> {
    pub(crate) const fn new(message_id: MessageId, operation: O::RequestData) -> Self {
        Self {
            message_id,
            operation,
        }
    }
}

impl<O: Operation> ToXml for Request<O> {
    type Error = crate::Error;

    fn to_xml(&self) -> Result<String, Self::Error> {
        Ok(quick_xml::se::to_string_with_root("rpc", self)?)
    }
}

impl<O: Operation> ClientMsg for Request<O> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartialReply {
    message_id: MessageId,
    buf: Box<str>,
}

impl PartialReply {
    pub(crate) const fn message_id(&self) -> MessageId {
        self.message_id
    }
}

impl FromXml for PartialReply {
    type Error = crate::Error;

    #[tracing::instrument]
    fn from_xml<S>(input: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug,
    {
        let mut reader = Reader::from_str(input.as_ref());
        _ = reader.trim_text(true);
        tracing::debug!("expecting <rpc-reply>");
        let message_id = match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"rpc-reply" => {
                tracing::debug!("trying to parse message-id");
                tag.try_get_attribute("message-id")?
                    .ok_or_else(|| crate::Error::NoMessageId)
                    .and_then(MessageId::try_from)
            }
            _ => Err(crate::Error::XmlParse(None)),
        }?;
        Ok(Self {
            message_id,
            buf: input.as_ref().trim_end_matches("]]>").into(),
        })
    }
}

#[async_trait]
impl ServerMsg for PartialReply {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply<O: Operation> {
    message_id: MessageId,
    inner: ReplyInner<O::ReplyData>,
}

impl<O: Operation> Reply<O> {
    pub(crate) fn into_result(self) -> Result<Option<O::ReplyData>, crate::Error> {
        // TODO:
        // Find a way to unify `Ok` and `Data` without wrapping in an `Option`
        match self.inner {
            ReplyInner::Ok => Ok(None),
            ReplyInner::Data(data) => Ok(Some(data)),
            ReplyInner::RpcError(err) => Err(err.into()),
        }
    }
}

impl<O: Operation> FromXml for Reply<O> {
    type Error = crate::Error;

    #[tracing::instrument]
    fn from_xml<S>(input: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug,
    {
        let mut reader = Reader::from_str(input.as_ref());
        _ = reader.trim_text(true);
        tracing::debug!("expecting <rpc-reply>");
        match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"rpc-reply" => {
                tracing::debug!("trying to parse message-id");
                let message_id = tag
                    .try_get_attribute("message-id")?
                    .ok_or_else(|| crate::Error::NoMessageId)
                    .and_then(MessageId::try_from)?;
                let end = tag.to_end();
                let span = reader.read_text(end.name())?;
                let inner = ReplyInner::from_xml(span)?;
                tracing::debug!("expecting eof");
                match reader.read_event()? {
                    Event::Eof => {
                        tracing::debug!(?message_id, ?inner);
                        Ok(Self { message_id, inner })
                    }
                    event => {
                        tracing::error!(?event, "unexpected xml event");
                        Err(crate::Error::XmlParse(None))
                    }
                }
            }
            event => {
                tracing::error!(?event, "unexpected xml event");
                Err(crate::Error::XmlParse(None))
            }
        }
    }
}

impl<O: Operation> TryFrom<PartialReply> for Reply<O> {
    type Error = crate::Error;

    #[tracing::instrument]
    fn try_from(value: PartialReply) -> Result<Self, Self::Error> {
        let this = Self::from_xml(&value.buf)?;
        if this.message_id == value.message_id {
            Ok(this)
        } else {
            Err(crate::Error::MessageIdMismatch(
                value.message_id,
                this.message_id,
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplyInner<D> {
    Ok,
    Data(D),
    RpcError(RpcError),
}

impl<D: FromXml + Debug> FromXml for ReplyInner<D> {
    type Error = crate::Error;

    #[tracing::instrument]
    fn from_xml<S>(input: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug,
    {
        let mut reader = Reader::from_str(input.as_ref());
        _ = reader.trim_text(true);
        tracing::debug!("expecting inner tag");
        let this = match reader.read_event()? {
            Event::Start(tag) if tag.name().as_ref() == b"data" => {
                tracing::debug!(?tag);
                reader
                    .read_text(tag.to_end().name())
                    .map_err(crate::Error::from)
                    .and_then(|span| {
                        D::from_xml(span)
                            .map_err(|err| crate::Error::RpcReplyDeserialization(err.into()))
                    })
                    .map(Self::Data)?
            }
            Event::Empty(tag) if tag.name().as_ref() == b"ok" => {
                tracing::debug!(?tag);
                Self::Ok
            }
            Event::Start(tag) if tag.name().as_ref() == b"rpc-error" => {
                tracing::debug!(?tag);
                reader
                    .read_text(tag.to_end().name())
                    .map_err(crate::Error::from)
                    .and_then(RpcError::from_xml)
                    .map(Self::RpcError)?
            }
            event => {
                tracing::error!(?event, "unexpected xml event");
                return Err(crate::Error::XmlParse(None));
            }
        };
        tracing::debug!("expecting eof");
        if matches!(reader.read_event()?, Event::Eof) {
            tracing::debug!(?this);
            Ok(this)
        } else {
            Err(crate::Error::XmlParse(None))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Empty {}

impl FromXml for Empty {
    type Error = Infallible;

    fn from_xml<S>(_: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug,
    {
        unreachable!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError;

impl FromXml for RpcError {
    type Error = crate::Error;

    fn from_xml<S>(_input: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug,
    {
        todo!()
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for RpcError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Foo;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    struct FooOperation {
        foo: &'static str,
    }

    impl Operation for Foo {
        type RequestData = FooOperation;
        type ReplyData = Empty;
    }

    #[test]
    fn serialize_foo_request() {
        let req: Request<Foo> = Request {
            message_id: MessageId(101),
            operation: FooOperation { foo: "bar" },
        };
        let expect = r#"<rpc message-id="101"><foo>bar</foo></rpc>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn deserialize_foo_reply() {
        let data = r#"
            <rpc-reply message-id="101">
                <ok/>
            </rpc-reply>
        "#;
        let expect: Reply<Foo> = Reply {
            message_id: MessageId(101),
            inner: ReplyInner::Ok,
        };
        assert_eq!(expect, Reply::from_xml(data).unwrap());
    }

    #[test]
    fn deserialize_ok_partial_reply() {
        let data = r#"
            <rpc-reply message-id="101">
                <ok/>
            </rpc-reply>
        "#;
        let expect = PartialReply {
            message_id: MessageId(101),
            buf: data.into(),
        };
        assert_eq!(expect, PartialReply::from_xml(data).unwrap());
    }

    #[test]
    fn deserialize_data_partial_reply() {
        let data = r#"
            <rpc-reply message-id="101">
                <data><foo/></data>
            </rpc-reply>
        "#;
        let expect = PartialReply {
            message_id: MessageId(101),
            buf: data.into(),
        };
        assert_eq!(expect, PartialReply::from_xml(data).unwrap());
    }
}
