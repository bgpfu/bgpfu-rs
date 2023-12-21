use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashSet},
    io::Write,
};

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
                    _ = inner.insert(Capability::from_uri(span.borrow())?);
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

mod uri {
    pub(super) const BASE_V1_0: &str = "urn:ietf:params:netconf:base:1.0";
    pub(super) const BASE_V1_1: &str = "urn:ietf:params:netconf:base:1.1";
    pub(super) const WRITABLE_RUNNING_V1_0: &str =
        "urn:ietf:params:netconf:capability:writable-running:1.0";
    pub(super) const CANDIDATE_V1_0: &str = "urn:ietf:params:netconf:capability:candidate:1.0";
    pub(super) const CONFIRMED_COMMIT_V1_0: &str =
        "urn:ietf:params:netconf:capability:confirmed-commit:1.0";
    pub(super) const ROLLBACK_ON_ERROR_V1_0: &str =
        "urn:ietf:params:netconf:capability:rollback-on-error:1.0";
    pub(super) const VALIDATE_V1_0: &str = "urn:ietf:params:netconf:capability:validate:1.0";
    pub(super) const STARTUP_V1_0: &str = "urn:ietf:params:netconf:capability:startup:1.0";
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
    Unknown(String),
}

impl Capability {
    #[tracing::instrument(level = "debug")]
    pub fn from_uri(uri: &str) -> Result<Self, Error> {
        match uri {
            uri::BASE_V1_0 => Ok(Self::Base(Base::V1_0)),
            uri::BASE_V1_1 => Ok(Self::Base(Base::V1_1)),
            uri::WRITABLE_RUNNING_V1_0 => Ok(Self::WritableRunning),
            uri::CANDIDATE_V1_0 => Ok(Self::Candidate),
            uri::CONFIRMED_COMMIT_V1_0 => Ok(Self::ConfirmedCommit),
            uri::ROLLBACK_ON_ERROR_V1_0 => Ok(Self::RollbackOnError),
            uri::VALIDATE_V1_0 => Ok(Self::Validate),
            uri::STARTUP_V1_0 => Ok(Self::Startup),
            _ => Ok(Self::Unknown(uri.to_string())),
        }
    }

    #[must_use]
    pub fn uri(&self) -> &str {
        match self {
            Self::Base(base) => base.uri(),
            Self::WritableRunning => uri::WRITABLE_RUNNING_V1_0,
            Self::Candidate => uri::CANDIDATE_V1_0,
            Self::ConfirmedCommit => uri::CONFIRMED_COMMIT_V1_0,
            Self::RollbackOnError => uri::ROLLBACK_ON_ERROR_V1_0,
            Self::Validate => uri::VALIDATE_V1_0,
            Self::Startup => uri::STARTUP_V1_0,
            Self::Unknown(uri) => uri.as_str(),
        }
    }
}

impl WriteXml for Capability {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("capability")
            .write_text_content(BytesText::new(self.uri()))?;
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
            Self::V1_0 => uri::BASE_V1_0,
            Self::V1_1 => uri::BASE_V1_1,
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
