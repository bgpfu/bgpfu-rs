use std::{
    convert::Infallible,
    fmt::{self, Debug},
    str::{from_utf8, FromStr},
    sync::Arc,
};

use quick_xml::{
    events::{BytesStart, Event},
    name::ResolveResult,
    NsReader,
};

use super::{xmlns, ReadXml};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Errors {
    inner: Vec<Error>,
}

impl Errors {
    pub(super) const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(super) fn push(&mut self, err: Error) {
        self.inner.push(err);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Error> {
        self.inner.iter()
    }
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.iter().try_for_each(|err| writeln!(f, "{err}"))
    }
}

impl std::error::Error for Errors {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    error_type: Type,
    error_tag: Tag,
    severity: Severity,
    app_tag: Option<AppTag>,
    path: Option<Path>,
    message: Option<Message>,
    info: Info,
}

impl ReadXml for Error {
    type Error = crate::Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let end = start.to_end();
        let mut error_type = None;
        let mut error_tag = None;
        let mut severity = None;
        let mut app_tag = None;
        let mut path = None;
        let mut message = None;
        let mut info = None;
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-type"
                        && error_type.is_none() =>
                {
                    tracing::debug!(?tag);
                    error_type = Some(reader.read_text(tag.to_end().name())?.trim().parse()?);
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-tag"
                        && error_tag.is_none() =>
                {
                    tracing::debug!(?tag);
                    error_tag = Some(reader.read_text(tag.to_end().name())?.trim().parse()?);
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-severity"
                        && severity.is_none() =>
                {
                    tracing::debug!(?tag);
                    severity = Some(reader.read_text(tag.to_end().name())?.trim().parse()?);
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-app-tag"
                        && app_tag.is_none() =>
                {
                    tracing::debug!(?tag);
                    app_tag = Some(
                        reader
                            .read_text(tag.to_end().name())?
                            .trim()
                            .parse()
                            .unwrap_or_else(|_| unreachable!()),
                    );
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-path"
                        && path.is_none() =>
                {
                    tracing::debug!(?tag);
                    path = Some(
                        reader
                            .read_text(tag.to_end().name())?
                            .trim()
                            .parse()
                            .unwrap_or_else(|_| unreachable!()),
                    );
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-message"
                        && message.is_none() =>
                {
                    tracing::debug!(?tag);
                    message = Some(
                        reader
                            .read_text(tag.to_end().name())?
                            .trim()
                            .parse()
                            .unwrap_or_else(|_| unreachable!()),
                    );
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"error-info"
                        && info.is_none() =>
                {
                    tracing::debug!(?tag);
                    info = Some(Info::read_xml(reader, &tag)?);
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(crate::Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        Ok(Self {
            error_type: error_type
                .ok_or_else(|| crate::Error::MissingElement("rpc-error", "<error-type>"))?,
            error_tag: error_tag
                .ok_or_else(|| crate::Error::MissingElement("rpc-error", "<error-tag>"))?,
            severity: severity
                .ok_or_else(|| crate::Error::MissingElement("rpc-error", "<error-severity>"))?,
            app_tag,
            path,
            message,
            info: info.unwrap_or_else(Info::new),
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.error_type, self.severity, self.error_tag
        )
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Type {
    Transport,
    Rpc,
    Protocol,
    Application,
}

impl FromStr for Type {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "transport" => Ok(Self::Transport),
            "rpc" => Ok(Self::Rpc),
            "protocol" => Ok(Self::Protocol),
            "application" => Ok(Self::Application),
            _ => Err(Self::Err::UnknownErrorType(s.to_string())),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = match self {
            Self::Transport => "transport",
            Self::Rpc => "rpc",
            Self::Protocol => "protocol",
            Self::Application => "application",
        };
        f.write_str(ty)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Tag {
    InUse,
    InvalidValue,
    TooBig,
    MissingAttribute,
    BadAttribute,
    UnknownAttribute,
    MissingElement,
    BadElement,
    UnknownElement,
    UnknownNamespace,
    AccessDenied,
    LockDenied,
    ResourceDenied,
    RollbackFailed,
    DataExists,
    DataMissing,
    OperationNotSupported,
    OperationFailed,
    MalformedMessage,

    // Deprecated:
    PartialOperation,
}

impl FromStr for Tag {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "in-use" => Ok(Self::InUse),
            "invalid-value" => Ok(Self::InvalidValue),
            "too-big" => Ok(Self::TooBig),
            "missing-attribute" => Ok(Self::MissingAttribute),
            "bad-attribute" => Ok(Self::BadAttribute),
            "unknown-attribute" => Ok(Self::UnknownAttribute),
            "missing-element" => Ok(Self::MissingElement),
            "bad-element" => Ok(Self::BadElement),
            "unknown-element" => Ok(Self::UnknownElement),
            "unknown-namespace" => Ok(Self::UnknownNamespace),
            "access-denied" => Ok(Self::AccessDenied),
            "lock-denied" => Ok(Self::LockDenied),
            "resource-denied" => Ok(Self::ResourceDenied),
            "rollback-failed" => Ok(Self::RollbackFailed),
            "data-exists" => Ok(Self::DataExists),
            "data-missing" => Ok(Self::DataMissing),
            "operation-not-supported" => Ok(Self::OperationNotSupported),
            "operation-failed" => Ok(Self::OperationFailed),
            "malformed-message" => Ok(Self::MalformedMessage),
            "partial-operation" => Ok(Self::PartialOperation),
            _ => Err(Self::Err::UnknownErrorTag(s.to_string())),
        }
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::InUse => "The request requires a resource that already is in use",
            Self::InvalidValue => "The request specifies an unacceptable value for one or more parameters",
            Self::TooBig => "The request or response (that would be generated) is too large for the implementation to handle",
            Self::MissingAttribute => "An expected attribute is missing",
            Self::BadAttribute => "An attribute value is not correct; e.g., wrong type, out of range, pattern mismatch",
            Self::UnknownAttribute => "An unexpected attribute is present",
            Self::MissingElement => "An expected element is missing",
            Self::BadElement => "An element value is not correct; e.g., wrong type, out of range, pattern mismatch",
            Self::UnknownElement => "An unexpected element is present",
            Self::UnknownNamespace => "An unexpected namespace is present",
            Self::AccessDenied => "Access to the requested protocol operation or data model is denied because authorization failed",
            Self::LockDenied => "Access to the requested lock is denied because the lock is currently held by another entity",
            Self::ResourceDenied => "Request could not be completed because of insufficient resources",
            Self::RollbackFailed => "Request to roll back some configuration change (via rollback-on-error or <discard-changes> operations) was not completed for some reason",
            Self::DataExists => "Request could not be completed because the relevant data model content already exists",
            Self::DataMissing => "Request could not be completed because the relevant data model content does not exist",
            Self::OperationNotSupported => "Request could not be completed because the requested operation is not supported by this implementation",
            Self::OperationFailed => "Request could not be completed because the requested operation failed for some reason not covered by any other error condition",
            Self::MalformedMessage => "A message could not be handled because it failed to be parsed correctly",
            Self::PartialOperation => "Some part of the requested operation failed or was not attempted for some reason",
        };
        f.write_str(msg)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

impl FromStr for Severity {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            _ => Err(Self::Err::UnknownErrorSeverity(s.to_string())),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let severity = match self {
            Self::Error => "error",
            Self::Warning => "warning",
        };
        f.write_str(severity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTag {
    inner: Arc<str>,
}

impl FromStr for AppTag {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { inner: s.into() })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    inner: Arc<str>,
}

impl FromStr for Path {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self { inner: s.into() })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    // TODO
    lang: (),
    inner: Arc<str>,
}

impl FromStr for Message {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            lang: (),
            inner: s.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    inner: Vec<InfoElement>,
}

impl Info {
    const fn new() -> Self {
        Self { inner: Vec::new() }
    }
}

impl ReadXml for Info {
    type Error = crate::Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let end = start.to_end();
        let mut inner = Vec::new();
        tracing::debug!("expecting error-info element");
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag)) if ns == xmlns::BASE => {
                    match tag.name().as_ref() {
                        b"bad-attribute" => inner.push(InfoElement::BadAttribute(
                            reader.read_text(tag.to_end().name())?.as_ref().into(),
                        )),
                        b"bad-element" => inner.push(InfoElement::BadElement(
                            reader.read_text(tag.to_end().name())?.as_ref().into(),
                        )),
                        b"bad-namespace" => inner.push(InfoElement::BadNamespace(
                            reader.read_text(tag.to_end().name())?.as_ref().into(),
                        )),
                        b"session-id" => inner.push(InfoElement::SessionId(
                            reader.read_text(tag.to_end().name())?.as_ref().parse()?,
                        )),
                        b"ok-element" => inner.push(InfoElement::OkElement(
                            reader.read_text(tag.to_end().name())?.as_ref().into(),
                        )),
                        b"err-element" => inner.push(InfoElement::ErrElement(
                            reader.read_text(tag.to_end().name())?.as_ref().into(),
                        )),
                        b"noop-element" => inner.push(InfoElement::NoopElement(
                            reader.read_text(tag.to_end().name())?.as_ref().into(),
                        )),
                        name => {
                            return Err(Self::Error::UnknownErrorInfo(from_utf8(name)?.to_string()))
                        }
                    }
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(crate::Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        Ok(Self { inner })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoElement {
    BadAttribute(Arc<str>),
    BadElement(Arc<str>),
    BadNamespace(Arc<str>),
    // TODO: SessionId should be a newtype containing a `usize`
    SessionId(usize),

    // Deprecated error-info elements:
    OkElement(Arc<str>),
    ErrElement(Arc<str>),
    NoopElement(Arc<str>),
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::message::{
        rpc::{Empty, MessageId, Operation, PartialReply, Reply, ReplyInner},
        ServerMsg, ToXml,
    };

    #[derive(Debug, PartialEq)]
    struct Dummy;

    impl Operation for Dummy {
        type ReplyData = Empty;
    }

    impl ToXml for Dummy {
        type Error = crate::Error;
        fn write_xml<W: Write>(&self, _: &mut W) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[test]
    fn deserialize_error_reply_rfc6241_s1_example1() {
        // message-id has been added, as we do not support it's omission
        let data = r#"
            <rpc-reply message-id="101"
              xmlns="urn:ietf:params:xml:ns:netconf:base:1.0">
              <rpc-error>
                <error-type>rpc</error-type>
                <error-tag>missing-attribute</error-tag>
                <error-severity>error</error-severity>
                <error-info>
                  <bad-attribute>message-id</bad-attribute>
                  <bad-element>rpc</bad-element>
                </error-info>
              </rpc-error>
            </rpc-reply>
        "#;
        let expect: Reply<Dummy> = Reply {
            message_id: MessageId(101),
            inner: ReplyInner::RpcError(Errors {
                inner: vec![Error {
                    error_type: Type::Rpc,
                    error_tag: Tag::MissingAttribute,
                    severity: Severity::Error,
                    app_tag: None,
                    path: None,
                    message: None,
                    info: Info {
                        inner: vec![
                            InfoElement::BadAttribute("message-id".into()),
                            InfoElement::BadElement("rpc".into()),
                        ],
                    },
                }],
            }),
        };
        assert_eq!(
            expect,
            PartialReply::from_xml(data)
                .and_then(Reply::try_from)
                .unwrap()
        );
    }
    #[test]
    fn deserialize_error_reply_rfc6241_s1_example2() {
        let data = r#"
            <rpc-reply message-id="101"
              xmlns="urn:ietf:params:xml:ns:netconf:base:1.0"
              xmlns:xc="urn:ietf:params:xml:ns:netconf:base:1.0">
              <rpc-error>
                <error-type>application</error-type>
                <error-tag>invalid-value</error-tag>
                <error-severity>error</error-severity>
                <error-path xmlns:t="http://example.com/schema/1.2/config">
                  /t:top/t:interface[t:name="Ethernet0/0"]/t:mtu
                </error-path>
                <error-message xml:lang="en">
                  MTU value 25000 is not within range 256..9192
                </error-message>
              </rpc-error>
              <rpc-error>
                <error-type>application</error-type>
                <error-tag>invalid-value</error-tag>
                <error-severity>error</error-severity>
                <error-path xmlns:t="http://example.com/schema/1.2/config">
                  /t:top/t:interface[t:name="Ethernet1/0"]/t:address/t:name
                </error-path>
                <error-message xml:lang="en">
                  Invalid IP address for interface Ethernet1/0
                </error-message>
              </rpc-error>
            </rpc-reply>
        "#;
        let expect: Reply<Dummy> = Reply {
            message_id: MessageId(101),
            inner: ReplyInner::RpcError(Errors {
                inner: vec![
                    Error {
                        error_type: Type::Application,
                        error_tag: Tag::InvalidValue,
                        severity: Severity::Error,
                        app_tag: None,
                        path: Some(Path {
                            inner: r#"/t:top/t:interface[t:name="Ethernet0/0"]/t:mtu"#.into(),
                        }),
                        message: Some(Message {
                            lang: (),
                            inner: "MTU value 25000 is not within range 256..9192".into(),
                        }),
                        info: Info::new(),
                    },
                    Error {
                        error_type: Type::Application,
                        error_tag: Tag::InvalidValue,
                        severity: Severity::Error,
                        app_tag: None,
                        path: Some(Path {
                            inner: r#"/t:top/t:interface[t:name="Ethernet1/0"]/t:address/t:name"#
                                .into(),
                        }),
                        message: Some(Message {
                            lang: (),
                            inner: "Invalid IP address for interface Ethernet1/0".into(),
                        }),
                        info: Info::new(),
                    },
                ],
            }),
        };
        assert_eq!(
            expect,
            PartialReply::from_xml(data)
                .and_then(Reply::try_from)
                .unwrap()
        );
    }
}
