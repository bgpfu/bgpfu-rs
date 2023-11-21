use std::{convert::Infallible, fmt::Debug, io::Write};

use async_trait::async_trait;
use quick_xml::{
    events::{attributes::Attribute, Event},
    Reader, Writer,
};

use super::{ClientMsg, FromXml, ServerMsg, ToXml, MARKER};

pub mod error;
pub use self::error::{Error, Errors};

pub mod operation;
pub use self::operation::Operation;

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub(crate) struct Request<O: Operation> {
    message_id: MessageId,
    operation: O,
}

impl<O: Operation> Request<O> {
    pub(crate) const fn new(message_id: MessageId, operation: O) -> Self {
        Self {
            message_id,
            operation,
        }
    }
}

impl<O: Operation> ToXml for Request<O> {
    type Error = crate::Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("rpc")
            .with_attribute(("message-id", self.message_id.0.to_string().as_ref()))
            .write_inner_content(|writer| {
                self.operation
                    .write_xml(writer.get_mut())
                    .map_err(|err| Self::Error::RpcRequestSerialization(err.into()))
            })?;
        Ok(())
    }
}

impl<O: Operation> ClientMsg for Request<O> {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartialReply {
    message_id: MessageId,
    inner_buf: Box<str>,
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
        let (mut message_id, mut inner_buf) = (None, None);
        tracing::debug!("expecting <rpc-reply>");
        loop {
            match reader.read_event()? {
                Event::Start(tag) if tag.name().as_ref() == b"rpc-reply" => {
                    tracing::debug!("trying to parse message-id");
                    message_id = Some(
                        tag.try_get_attribute("message-id")?
                            .ok_or_else(|| crate::Error::NoMessageId)
                            .and_then(MessageId::try_from)?,
                    );
                    inner_buf = Some(
                        reader
                            .read_text(tag.to_end().name())?
                            .as_ref()
                            .trim()
                            .into(),
                    );
                }
                Event::Comment(_) => continue,
                Event::Eof => break,
                Event::Text(txt) if txt.as_ref() == MARKER => break,
                event => {
                    tracing::error!(?event, "unexpected xml event");
                    return Err(crate::Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        Ok(Self {
            message_id: message_id.ok_or_else(|| crate::Error::NoMessageId)?,
            // If `message_id` is `Some`, and we haven't already encountered an error, then
            // `inner_buf` is also guaranteed to be `Some` here.
            inner_buf: inner_buf.expect("inner_buf should be initialized"),
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
            ReplyInner::RpcError(errors) => Err(errors.into()),
        }
    }
}

impl<O: Operation> TryFrom<PartialReply> for Reply<O> {
    type Error = crate::Error;

    #[tracing::instrument]
    fn try_from(value: PartialReply) -> Result<Self, Self::Error> {
        let message_id = value.message_id;
        let inner = ReplyInner::from_xml(value.inner_buf)?;
        Ok(Self { message_id, inner })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplyInner<D> {
    Ok,
    Data(D),
    RpcError(Errors),
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
        let mut errors = Errors::new();
        let mut this = None;
        tracing::debug!("expecting inner tag");
        loop {
            match reader.read_event()? {
                Event::Start(tag)
                    if tag.name().as_ref() == b"data" && this.is_none() && errors.is_empty() =>
                {
                    tracing::debug!(?tag);
                    this = reader
                        .read_text(tag.to_end().name())
                        .map_err(crate::Error::from)
                        .and_then(|span| {
                            D::from_xml(span)
                                .map_err(|err| crate::Error::RpcReplyDeserialization(err.into()))
                        })
                        .map(Self::Data)
                        .map(Some)?;
                }
                Event::Empty(tag)
                    if tag.name().as_ref() == b"ok" && this.is_none() && errors.is_empty() =>
                {
                    tracing::debug!(?tag);
                    this = Some(Self::Ok);
                }
                Event::Start(tag) if tag.name().as_ref() == b"rpc-error" && this.is_none() => {
                    tracing::debug!(?tag);
                    let error = reader
                        .read_text(tag.to_end().name())
                        .map_err(crate::Error::from)
                        .and_then(Error::from_xml)?;
                    errors.push(error);
                }
                Event::Comment(_) => continue,
                Event::Eof => break,
                event => {
                    tracing::error!(?event, "unexpected xml event");
                    return Err(crate::Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        this.or_else(|| (!errors.is_empty()).then(|| Self::RpcError(errors)))
            .ok_or_else(|| crate::Error::MissingElement("rpc-reply", "<data>/<ok>/<rpc-error>"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[cfg(test)]
mod tests {
    use quick_xml::events::BytesText;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Foo {
        foo: &'static str,
    }

    impl ToXml for Foo {
        type Error = crate::Error;

        fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
            _ = Writer::new(writer)
                .create_element("foo")
                .write_text_content(BytesText::new(self.foo))?;
            Ok(())
        }
    }

    impl Operation for Foo {
        type ReplyData = Empty;
    }

    #[test]
    fn serialize_foo_request() {
        let req: Request<Foo> = Request {
            message_id: MessageId(101),
            operation: Foo { foo: "bar" },
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
        assert_eq!(
            expect,
            PartialReply::from_xml(data)
                .and_then(Reply::try_from)
                .unwrap()
        );
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
            inner_buf: "<ok/>".into(),
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
            inner_buf: "<data><foo/></data>".into(),
        };
        assert_eq!(expect, PartialReply::from_xml(data).unwrap());
    }
}
