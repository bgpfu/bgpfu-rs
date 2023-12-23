use std::{
    borrow::Cow,
    collections::{BTreeSet, HashSet},
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
    pub fn contains(&self, elem: &Capability) -> bool {
        self.inner.contains(elem)
    }

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
    ConfirmedCommit,
    RollbackOnError,
    Validate,
    Startup,
    Url(Vec<Box<str>>),
    Unknown(Arc<UriStr>),
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
                Ok(Self::ConfirmedCommit)
            }
            ("urn", None, "ietf:params:netconf:capability:rollback-on-error:1.0", None, None) => {
                Ok(Self::RollbackOnError)
            }
            ("urn", None, "ietf:params:netconf:capability:validate:1.0", None, None) => {
                Ok(Self::Validate)
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
            Self::ConfirmedCommit => {
                Cow::Borrowed("urn:ietf:params:netconf:capability:confirmed-commit:1.0")
            }
            Self::RollbackOnError => {
                Cow::Borrowed("urn:ietf:params:netconf:capability:rollback-on-error:1.0")
            }
            Self::Validate => Cow::Borrowed("urn:ietf:params:netconf:capability:validate:1.0"),
            Self::Startup => Cow::Borrowed("urn:ietf:params:netconf:capability:startup:1.0"),
            // TODO
            Self::Url(schemes) => Cow::Owned(format!(
                "urn:ietf:params:netconf:capability:url:1.0?scheme={}",
                schemes.join(",")
            )),
            Self::Unknown(uri) => Cow::Borrowed(uri.as_str()),
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
