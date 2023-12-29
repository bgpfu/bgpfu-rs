use std::{
    borrow::Cow,
    collections::{BTreeSet, HashSet},
    fmt::Display,
    io::Write,
    str::FromStr,
    sync::Arc,
};

use iri_string::types::UriStr;
use quick_xml::{
    events::{BytesStart, BytesText, Event},
    name::ResolveResult,
    NsReader, Writer,
};

use crate::{
    message::{xmlns, ReadXml, WriteXml},
    Error,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    inner: HashSet<Capability>,
}

impl Capabilities {
    pub fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.inner.iter()
    }

    #[tracing::instrument(ret, level = "debug")]
    fn contains(&self, elem: &Capability) -> bool {
        self.inner.contains(elem)
    }

    // #[tracing::instrument(ret, level = "debug")]
    // pub fn contains_any(&self, elems: &[&Capability]) -> bool {
    //     elems.iter().any(|elem| self.contains(elem))
    // }
    //
    #[tracing::instrument(ret, level = "debug")]
    pub(crate) fn highest_common_version(&self, other: &Self) -> Result<Base, Error> {
        self.inner
            .intersection(&other.inner)
            .filter_map(|capability| {
                if let Capability::Base(base) = capability {
                    Some(*base)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>()
            .last()
            .ok_or_else(|| Error::VersionNegotiation)
            .copied()
    }
}

impl ReadXml for Capabilities {
    type Error = Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let mut inner = HashSet::new();
        let end = start.to_end();
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE && tag.local_name().as_ref() == b"capability" =>
                {
                    let span = reader.read_text(tag.to_end().name())?;
                    _ = inner.insert(span.parse()?);
                }
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(Error::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        Ok(Self { inner })
    }
}

impl WriteXml for Capabilities {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("capabilities")
            .write_inner_content(|writer| {
                self.inner
                    .iter()
                    .try_for_each(|capability| capability.write_xml(writer.get_mut()))
            })?;
        Ok(())
    }
}

impl FromIterator<Capability> for Capabilities {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Capability>,
    {
        Self {
            inner: iter.into_iter().collect(),
        }
    }
}

#[allow(variant_size_differences)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Capability {
    Base(Base),
    WritableRunning,
    Candidate,
    ConfirmedCommitV1_0,
    ConfirmedCommitV1_1,
    RollbackOnError,
    ValidateV1_0,
    ValidateV1_1,
    Startup,
    Url(Vec<Box<str>>),
    XPath,
    Unknown(Arc<UriStr>),
    #[cfg(feature = "junos")]
    JunosXmlManagementProtocol,
}

impl FromStr for Capability {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let uri = UriStr::new(s)?;
        match (
            uri.scheme_str(),
            uri.authority_str(),
            uri.path_str(),
            uri.query_str(),
            uri.fragment(),
        ) {
            ("urn", None, "ietf:params:netconf:base:1.0", None, None) => Ok(Self::Base(Base::V1_0)),
            ("urn", None, "ietf:params:netconf:base:1.1", None, None) => Ok(Self::Base(Base::V1_1)),
            ("urn", None, "ietf:params:netconf:capability:writable-running:1.0", None, None) => {
                Ok(Self::WritableRunning)
            }
            ("urn", None, "ietf:params:netconf:capability:candidate:1.0", None, None) => {
                Ok(Self::Candidate)
            }
            ("urn", None, "ietf:params:netconf:capability:confirmed-commit:1.0", None, None) => {
                Ok(Self::ConfirmedCommitV1_0)
            }
            ("urn", None, "ietf:params:netconf:capability:confirmed-commit:1.1", None, None) => {
                Ok(Self::ConfirmedCommitV1_1)
            }
            ("urn", None, "ietf:params:netconf:capability:rollback-on-error:1.0", None, None) => {
                Ok(Self::RollbackOnError)
            }
            ("urn", None, "ietf:params:netconf:capability:validate:1.0", None, None) => {
                Ok(Self::ValidateV1_0)
            }
            ("urn", None, "ietf:params:netconf:capability:validate:1.1", None, None) => {
                Ok(Self::ValidateV1_1)
            }
            ("urn", None, "ietf:params:netconf:capability:startup:1.0", None, None) => {
                Ok(Self::Startup)
            }
            ("urn", None, "ietf:params:netconf:capability:url:1.0", Some(query), None) => {
                let schemes = query
                    .split('&')
                    .filter_map(|pair| match pair.split_once('=') {
                        Some(("scheme", values)) => Some(values.split(',')),
                        _ => None,
                    })
                    .flatten()
                    .map(Box::from)
                    .collect();
                Ok(Self::Url(schemes))
            }
            ("urn", None, "ietf:params:netconf:capability:xpath:1.0", None, None) => {
                Ok(Self::XPath)
            }
            #[cfg(feature = "junos")]
            ("http", Some("xml.juniper.net"), "/netconf/junos/1.0", None, None) => {
                Ok(Self::JunosXmlManagementProtocol)
            }
            _ => Ok(Self::Unknown(uri.into())),
        }
    }
}

impl Capability {
    #[must_use]
    pub fn uri(&self) -> Cow<'_, str> {
        match self {
            Self::Base(base) => Cow::Borrowed(base.uri()),
            Self::WritableRunning => {
                Cow::Borrowed("urn:ietf:params:netconf:capability:writable-running:1.0")
            }
            Self::Candidate => Cow::Borrowed("urn:ietf:params:netconf:capability:candidate:1.0"),
            Self::ConfirmedCommitV1_0 => {
                Cow::Borrowed("urn:ietf:params:netconf:capability:confirmed-commit:1.0")
            }
            Self::ConfirmedCommitV1_1 => {
                Cow::Borrowed("urn:ietf:params:netconf:capability:confirmed-commit:1.1")
            }
            Self::RollbackOnError => {
                Cow::Borrowed("urn:ietf:params:netconf:capability:rollback-on-error:1.0")
            }
            Self::ValidateV1_0 => Cow::Borrowed("urn:ietf:params:netconf:capability:validate:1.0"),
            Self::ValidateV1_1 => Cow::Borrowed("urn:ietf:params:netconf:capability:validate:1.1"),
            Self::Startup => Cow::Borrowed("urn:ietf:params:netconf:capability:startup:1.0"),
            Self::Url(schemes) => Cow::Owned(format!(
                "urn:ietf:params:netconf:capability:url:1.0?scheme={}",
                schemes.join(",")
            )),
            Self::XPath => Cow::Borrowed("urn:ietf:params:netconf:capability:xpath:1.0"),
            Self::Unknown(uri) => Cow::Borrowed(uri.as_str()),
            #[cfg(feature = "junos")]
            Self::JunosXmlManagementProtocol => {
                Cow::Borrowed("http://xml.juniper.net/netconf/junos/1.0")
            }
        }
    }
}

impl WriteXml for Capability {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("capability")
            .write_text_content(BytesText::new(&self.uri()))?;
        Ok(())
    }
}

impl Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Base(base) => base.fmt(f),
            Self::WritableRunning => f.write_str(":writable-running:1.0"),
            Self::Candidate => f.write_str(":candidate:1.0"),
            Self::ConfirmedCommitV1_0 => f.write_str(":confirmed-commit:1.0"),
            Self::ConfirmedCommitV1_1 => f.write_str(":confirmed-commit:1.1"),
            Self::RollbackOnError => f.write_str(":rollback-on-error:1.0"),
            Self::ValidateV1_0 => f.write_str(":validate:1.0"),
            Self::ValidateV1_1 => f.write_str(":validate:1.1"),
            Self::Url(schemes) => write!(f, ":url:1.0?scheme={}", schemes.join(",")),
            Self::XPath => f.write_str(":xpath:1.0"),
            _ => f.write_str(&self.uri()),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Base {
    V1_0,
    V1_1,
}

impl Base {
    const fn uri(&self) -> &str {
        match self {
            Self::V1_0 => "urn:ietf:params:netconf:base:1.0",
            Self::V1_1 => "urn:ietf:params:netconf:base:1.1",
        }
    }
}

impl Display for Base {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::V1_0 => f.write_str(":base:1.0"),
            Self::V1_1 => f.write_str(":base:1.1"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Requirements {
    None,
    One(Capability),
    Any(&'static [Capability]),
    All(&'static [Capability]),
}

impl Requirements {
    pub(crate) fn check(&self, capabilities: &Capabilities) -> bool {
        match self {
            Self::None => true,
            Self::One(requirement) => capabilities.contains(requirement),
            Self::Any(requirements) => requirements
                .iter()
                .any(|requirement| capabilities.contains(requirement)),
            Self::All(requirements) => requirements
                .iter()
                .all(|requirement| capabilities.contains(requirement)),
        }
    }
}

impl Display for Requirements {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None | Self::Any(&[]) | Self::All(&[]) => f.write_str("none"),
            Self::One(ref capability)
            | Self::Any(&[ref capability])
            | Self::All(&[ref capability]) => {
                write!(f, "exactly '{capability}'")
            }
            Self::Any(requirements) => {
                let mut iter = requirements.iter();
                if let Some(first) = iter.next() {
                    write!(f, "any of '{first}'")?;
                };
                iter.try_for_each(|requirement| write!(f, ", '{requirement}'"))
            }
            Self::All(requirements) => {
                let mut iter = requirements.iter();
                if let Some(first) = iter.next() {
                    write!(f, "all of '{first}'")?;
                };
                iter.try_for_each(|requirement| write!(f, ", '{requirement}'"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::{empty, once};

    use super::*;

    #[test]
    fn empty_client_capabilities() {
        let client_capabilities: Capabilities = empty().collect();
        let server_capabilities: Capabilities = once(Capability::Base(Base::V1_1)).collect();
        let common = client_capabilities.highest_common_version(&server_capabilities);
        assert!(common.is_err());
    }

    #[test]
    fn empty_server_capabilities() {
        let client_capabilities: Capabilities = once(Capability::Base(Base::V1_0)).collect();
        let server_capabilities: Capabilities = empty().collect();
        let common = client_capabilities.highest_common_version(&server_capabilities);
        assert!(common.is_err());
    }

    #[test]
    fn no_common_base() {
        let client_capabilities: Capabilities = once(Capability::Base(Base::V1_0)).collect();
        let server_capabilities: Capabilities = once(Capability::Base(Base::V1_1)).collect();
        let common = client_capabilities.highest_common_version(&server_capabilities);
        assert!(common.is_err());
    }

    #[test]
    fn common_version_1_0() {
        let client_capabilities: Capabilities = once(Capability::Base(Base::V1_0)).collect();
        let server_capabilities: Capabilities =
            [Capability::Base(Base::V1_0), Capability::Base(Base::V1_1)]
                .into_iter()
                .collect();
        let common = client_capabilities.highest_common_version(&server_capabilities);
        assert_eq!(common.unwrap(), Base::V1_0);
    }

    #[test]
    fn common_version_1_1() {
        let client_capabilities: Capabilities =
            [Capability::Base(Base::V1_0), Capability::Base(Base::V1_1)]
                .into_iter()
                .collect();
        let server_capabilities: Capabilities = once(Capability::Base(Base::V1_1)).collect();
        let common = client_capabilities.highest_common_version(&server_capabilities);
        assert_eq!(common.unwrap(), Base::V1_1);
    }

    #[test]
    fn common_version_highest() {
        let client_capabilities: Capabilities =
            [Capability::Base(Base::V1_0), Capability::Base(Base::V1_1)]
                .into_iter()
                .collect();
        let server_capabilities: Capabilities =
            [Capability::Base(Base::V1_0), Capability::Base(Base::V1_1)]
                .into_iter()
                .collect();
        let common = client_capabilities.highest_common_version(&server_capabilities);
        assert_eq!(common.unwrap(), Base::V1_1);
    }
}
