use std::{convert::Infallible, fmt::Debug, io::Write, str::from_utf8};

use quick_xml::{
    events::{attributes::Attribute, BytesStart, Event},
    name::{Namespace, ResolveResult},
    NsReader, Writer,
};

use super::{xmlns, ClientMsg, ReadXml, ServerMsg, ToXml, MARKER};

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
    buf: Box<str>,
}

impl PartialReply {
    pub(crate) const fn message_id(&self) -> MessageId {
        self.message_id
    }
}

impl ServerMsg for PartialReply {
    const TAG_NS: Namespace<'static> = xmlns::BASE;
    const TAG_NAME: &'static str = "rpc-reply";

    // TODO:
    // This is a hack - we need to find a way to save the buffer without resorting to this trick
    // of calling `read_xml` with a dummy start tag
    #[tracing::instrument(skip(input))]
    fn from_xml<S>(input: S) -> Result<Self, crate::Error>
    where
        S: AsRef<str> + Debug,
    {
        tracing::debug!(?input);
        let mut reader = NsReader::from_str(input.as_ref());
        _ = reader.trim_text(true);
        Self::read_xml(&mut reader, &BytesStart::new("dummy"))
    }
}

impl ReadXml for PartialReply {
    type Error = crate::Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, _: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let buf = from_utf8(reader.get_ref())?.into();
        tracing::debug!("expecting <{}>", Self::TAG_NAME);
        let mut message_id = None;
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == Self::TAG_NS
                        && tag.local_name().as_ref() == Self::TAG_NAME.as_bytes() =>
                {
                    let end = tag.to_end();
                    tracing::debug!("trying to parse message-id");
                    message_id = tag
                        .try_get_attribute("message-id")?
                        .map(MessageId::try_from)
                        .transpose()?;
                    _ = reader.read_to_end(end.name());
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::Eof) => break,
                (_, Event::Text(txt)) if &*txt == MARKER => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(crate::Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        Ok(Self {
            message_id: message_id.ok_or_else(|| crate::Error::NoMessageId)?,
            buf,
        })
    }
}

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

impl<O: Operation> ReadXml for Reply<O> {
    type Error = crate::Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        tracing::debug!("trying to parse message-id");
        let message_id = start
            .try_get_attribute("message-id")?
            .ok_or_else(|| crate::Error::NoMessageId)
            .and_then(MessageId::try_from)?;
        let inner = ReplyInner::read_xml(reader, start)?;
        Ok(Self { message_id, inner })
    }
}

impl<O: Operation> ServerMsg for Reply<O> {
    const TAG_NS: Namespace<'static> = xmlns::BASE;
    const TAG_NAME: &'static str = "rpc-reply";
}

impl<O: Operation> TryFrom<PartialReply> for Reply<O> {
    type Error = crate::Error;

    #[tracing::instrument(err, level = "debug")]
    fn try_from(value: PartialReply) -> Result<Self, Self::Error> {
        let this = Self::from_xml(&value.buf)?;
        if this.message_id != value.message_id {
            return Err(crate::Error::MessageIdMismatch(
                this.message_id,
                value.message_id,
            ));
        };
        Ok(this)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplyInner<D> {
    Ok,
    Data(D),
    RpcError(Errors),
}

impl<D: ReadXml> ReadXml for ReplyInner<D> {
    type Error = crate::Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let end = start.to_end();
        let mut errors = Errors::new();
        let mut this = None;
        tracing::debug!("expecting <ok>, <data> or <rpc-error>");
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"data"
                        && this.is_none()
                        && errors.is_empty() =>
                {
                    tracing::debug!(?tag);
                    this = Some(Self::Data(
                        D::read_xml(reader, &tag)
                            .map_err(|err| crate::Error::ReadXml(err.into()))?,
                    ));
                }
                (ResolveResult::Bound(ns), Event::Empty(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"ok"
                        && this.is_none()
                        && errors.is_empty() =>
                {
                    tracing::debug!(?tag);
                    this = Some(Self::Ok);
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"rpc-error"
                        && this.is_none() =>
                {
                    tracing::debug!(?tag);
                    errors.push(Error::read_xml(reader, &tag)?);
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
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

impl ReadXml for Empty {
    type Error = Infallible;

    fn read_xml(_: &mut NsReader<&[u8]>, _: &BytesStart<'_>) -> Result<Self, Self::Error> {
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
        let expect = r#"<rpc message-id="101"><foo>bar</foo></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn deserialize_foo_reply() {
        let data = r#"
            <rpc-reply
                message-id="101"
                xmlns="urn:ietf:params:xml:ns:netconf:base:1.0">
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
    fn deserialize_foo_reply_with_xmlns() {
        let data = r#"
            <nc:rpc-reply
                message-id="101"
                xmlns:nc="urn:ietf:params:xml:ns:netconf:base:1.0"
                xmlns:junos="http://xml.juniper.net/junos/23.1R0/junos">
                <nc:ok/>
            </nc:rpc-reply>
            ]]>]]>
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Bar;

    impl ToXml for Bar {
        type Error = crate::Error;

        fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
            _ = Writer::new(writer).create_element("bar").write_empty()?;
            Ok(())
        }
    }

    impl Operation for Bar {
        type ReplyData = BarReply;
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct BarReply(usize);

    impl ReadXml for BarReply {
        type Error = crate::Error;

        fn read_xml(
            reader: &mut NsReader<&[u8]>,
            start: &BytesStart<'_>,
        ) -> Result<Self, Self::Error> {
            let end = start.to_end();
            let mut result = None;
            loop {
                match reader.read_resolved_event()? {
                    (ResolveResult::Bound(ns), Event::Start(tag))
                        if ns == Namespace(b"bar")
                            && tag.local_name().as_ref() == b"result"
                            && result.is_none() =>
                    {
                        result = Some(reader.read_text(tag.to_end().name())?.parse()?);
                    }
                    (_, Event::Comment(_)) => continue,
                    (_, Event::End(tag)) if tag == end => break,
                    (ns, event) => {
                        tracing::error!(?event, ?ns, "unexpected xml event");
                        return Err(crate::Error::UnexpectedXmlEvent(event.into_owned()));
                    }
                }
            }
            Ok(Self(result.ok_or_else(|| {
                crate::Error::MissingElement("rpc-reply", "<result>")
            })?))
        }
    }

    #[test]
    fn serialize_bar_request() {
        let req = Request {
            message_id: MessageId(101),
            operation: Bar,
        };
        let expect = r#"<rpc message-id="101"><bar/></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn deserialize_bar_reply() {
        let data = r#"
            <rpc-reply
                message-id="101"
                xmlns="urn:ietf:params:xml:ns:netconf:base:1.0">
                <data>
                    <result xmlns="bar">99</result>
                </data>
            </rpc-reply>
        "#;
        let expect: Reply<Bar> = Reply {
            message_id: MessageId(101),
            inner: ReplyInner::Data(BarReply(99)),
        };
        assert_eq!(
            expect,
            PartialReply::from_xml(data)
                .and_then(Reply::try_from)
                .unwrap()
        );
    }

    #[test]
    fn deserialize_bar_reply_with_xmlns() {
        let data = r#"
            <nc:rpc-reply
                message-id="101"
                xmlns:nc="urn:ietf:params:xml:ns:netconf:base:1.0"
                xmlns:junos="http://xml.juniper.net/junos/23.1R0/junos">
                <nc:data>
                    <bar:result xmlns:bar="bar">99</bar:result>
                </nc:data>
            </nc:rpc-reply>
            ]]>]]>
        "#;
        let expect: Reply<Bar> = Reply {
            message_id: MessageId(101),
            inner: ReplyInner::Data(BarReply(99)),
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
            <rpc-reply
                message-id="101"
                xmlns="urn:ietf:params:xml:ns:netconf:base:1.0">
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
            <rpc-reply
                message-id="101"
                xmlns="urn:ietf:params:xml:ns:netconf:base:1.0">
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
