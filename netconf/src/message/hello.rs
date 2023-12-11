use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    fmt::Debug,
    io::Write,
};

use quick_xml::{
    events::{BytesStart, BytesText, Event},
    name::{Namespace, ResolveResult},
    NsReader, Writer,
};

use crate::Error;

use super::{xmlns, ClientMsg, ReadXml, ServerMsg, WriteXml};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerHello {
    capabilities: Capabilities,
    session_id: usize,
}

impl ServerHello {
    pub(crate) const fn session_id(&self) -> usize {
        self.session_id
    }
}

impl ReadXml for ServerHello {
    type Error = Error;

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Self::Error> {
        let end = start.to_end();
        let (mut capabilities, mut session_id) = (None, None);
        tracing::debug!("expecting <capabilities> or <session-id>");
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"capabilities"
                        && capabilities.is_none() =>
                {
                    tracing::debug!("trying to deserialize capabilities");
                    capabilities = Some(Capabilities::read_xml(reader, &tag)?);
                }
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE
                        && tag.local_name().as_ref() == b"session-id"
                        && session_id.is_none() =>
                {
                    let span = reader.read_text(tag.to_end().name())?;
                    session_id = Some(span.parse()?);
                }
                (_, Event::Comment(_)) => {
                    continue;
                }
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(Error::UnexpectedXmlEvent(event.into_owned()));
                }
            };
        }
        Ok(Self {
            capabilities: capabilities
                .ok_or_else(|| Error::MissingElement("hello", "<capabilities>"))?,
            session_id: session_id.ok_or_else(|| Error::MissingElement("hello", "<session-id>"))?,
        })
    }
}

impl ServerMsg for ServerHello {
    const TAG_NAME: &'static str = "hello";
    const TAG_NS: Namespace<'static> = xmlns::BASE;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClientHello {
    capabilities: Capabilities,
}

impl ClientHello {
    #[tracing::instrument(level = "debug")]
    pub(crate) fn new(capabilities: &[Capability]) -> Self {
        Self {
            capabilities: capabilities.iter().cloned().collect(),
        }
    }

    #[tracing::instrument(err, level = "debug")]
    pub(crate) fn common_capabilities(
        &self,
        server_hello: &ServerHello,
    ) -> Result<Capabilities, Error> {
        let common = self
            .capabilities
            .inner
            .intersection(&server_hello.capabilities.inner)
            .cloned()
            .collect::<Capabilities>();
        if common.contains(&BASE) {
            Ok(common)
        } else {
            Err(Error::BaseCapability)
        }
    }
}

impl WriteXml for ClientHello {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("hello")
            .write_inner_content(|writer| self.capabilities.write_xml(writer.get_mut()))?;
        Ok(())
    }
}

impl ClientMsg for ClientHello {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Capabilities {
    inner: HashSet<Capability>,
}

impl Capabilities {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.inner.iter()
    }

    #[tracing::instrument(ret)]
    pub(crate) fn contains(&self, elem: &Capability) -> bool {
        self.inner.contains(elem)
    }

    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, Error> {
        let mut inner = HashSet::new();
        let end = start.to_end();
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(ns), Event::Start(tag))
                    if ns == xmlns::BASE && tag.local_name().as_ref() == b"capability" =>
                {
                    let span = reader.read_text(tag.to_end().name())?;
                    _ = inner.insert(Capability::new(span.to_string()));
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Capability {
    uri: Cow<'static, str>,
}

impl Capability {
    #[must_use]
    #[tracing::instrument(level = "debug")]
    pub fn new(uri: String) -> Self {
        Self {
            uri: Cow::Owned(uri),
        }
    }

    #[must_use]
    pub const fn new_static(uri: &'static str) -> Self {
        Self {
            uri: Cow::Borrowed(uri),
        }
    }

    #[must_use]
    pub fn uri(&self) -> &str {
        self.uri.borrow()
    }
}

impl WriteXml for Capability {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("capability")
            .write_text_content(BytesText::new(self.uri.as_ref()))?;
        Ok(())
    }
}

pub(crate) const BASE: Capability = Capability::new_static("urn:ietf:params:netconf:base:1.0");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_hello_from_xml() {
        let xml = r#"
            <hello xmlns="urn:ietf:params:xml:ns:netconf:base:1.0">
              <capabilities>
                <capability>urn:ietf:params:netconf:base:1.0</capability>
                <capability>urn:ietf:params:netconf:capability:candidate:1.0</capability>
                <capability>urn:ietf:params:netconf:capability:confirmed-commit:1.0</capability>
                <capability>urn:ietf:params:netconf:capability:validate:1.0</capability>
                <capability>urn:ietf:params:netconf:capability:url:1.0?scheme=http,ftp,file</capability>
                <capability>urn:ietf:params:xml:ns:netconf:base:1.0</capability>
                <capability>urn:ietf:params:xml:ns:netconf:capability:candidate:1.0</capability>
                <capability>urn:ietf:params:xml:ns:netconf:capability:confirmed-commit:1.0</capability>
                <capability>urn:ietf:params:xml:ns:netconf:capability:validate:1.0</capability>
                <capability>urn:ietf:params:xml:ns:netconf:capability:url:1.0?scheme=http,ftp,file</capability>
                <capability>urn:ietf:params:xml:ns:yang:ietf-netconf-monitoring</capability>
                <capability>http://xml.juniper.net/netconf/junos/1.0</capability>
                <capability>http://xml.juniper.net/dmi/system/1.0</capability>
              </capabilities>
              <session-id>802</session-id>
            </hello>
        "#;
        let expect = ServerHello {
            capabilities: Capabilities {
                inner: [
                    Capability::new_static("urn:ietf:params:netconf:base:1.0"),
                    Capability::new_static("urn:ietf:params:netconf:capability:candidate:1.0"),
                    Capability::new_static(
                        "urn:ietf:params:netconf:capability:confirmed-commit:1.0",
                    ),
                    Capability::new_static("urn:ietf:params:netconf:capability:validate:1.0"),
                    Capability::new_static(
                        "urn:ietf:params:netconf:capability:url:1.0?scheme=http,ftp,file",
                    ),
                    Capability::new_static("urn:ietf:params:xml:ns:netconf:base:1.0"),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:candidate:1.0",
                    ),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:confirmed-commit:1.0",
                    ),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:validate:1.0",
                    ),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:url:1.0?scheme=http,ftp,file",
                    ),
                    Capability::new_static("urn:ietf:params:xml:ns:yang:ietf-netconf-monitoring"),
                    Capability::new_static("http://xml.juniper.net/netconf/junos/1.0"),
                    Capability::new_static("http://xml.juniper.net/dmi/system/1.0"),
                ]
                .into(),
            },
            session_id: 802,
        };
        assert_eq!(expect, ServerHello::from_xml(xml).unwrap());
    }

    #[test]
    fn server_hello_with_xmlns_from_xml() {
        let xml = r#"
            <nc:hello xmlns:nc="urn:ietf:params:xml:ns:netconf:base:1.0">
               <nc:capabilities>
                <nc:capability>urn:ietf:params:netconf:base:1.0</nc:capability>
                <nc:capability>urn:ietf:params:netconf:capability:candidate:1.0</nc:capability>
                <nc:capability>urn:ietf:params:netconf:capability:confirmed-commit:1.0</nc:capability>
                <nc:capability>urn:ietf:params:netconf:capability:validate:1.0</nc:capability>
                <nc:capability>urn:ietf:params:netconf:capability:url:1.0?scheme=http,ftp,file</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:netconf:base:1.0?module=ietf-netconf&amp;revision=2011-06-01</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:netconf:capability:candidate:1.0</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:netconf:capability:confirmed-commit:1.0</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:netconf:capability:validate:1.0</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:netconf:capability:url:1.0?scheme=http,ftp,file</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:yang:ietf-inet-types?module=ietf-inet-types&amp;revision=2013-07-15</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:yang:ietf-yang-metadata?module=ietf-yang-metadata&amp;revision=2016-08-05</nc:capability>
                <nc:capability>urn:ietf:params:xml:ns:yang:ietf-netconf-monitoring</nc:capability>
                <nc:capability>http://xml.juniper.net/netconf/junos/1.0</nc:capability>
                <nc:capability>http://xml.juniper.net/dmi/system/1.0</nc:capability>
                <nc:capability>http://yang.juniper.net/junos/jcmd?module=junos-configuration-metadata&amp;revision=2021-09-01</nc:capability>
              </nc:capabilities>
              <nc:session-id>43129</nc:session-id>
            </nc:hello>
            ]]>]]>
        "#;
        let expect = ServerHello {
            capabilities: Capabilities {
                inner: [
                    Capability::new_static("urn:ietf:params:netconf:base:1.0"),
                    Capability::new_static("urn:ietf:params:netconf:capability:candidate:1.0"),
                    Capability::new_static(
                        "urn:ietf:params:netconf:capability:confirmed-commit:1.0",
                    ),
                    Capability::new_static("urn:ietf:params:netconf:capability:validate:1.0"),
                    Capability::new_static(
                        "urn:ietf:params:netconf:capability:url:1.0?scheme=http,ftp,file",
                    ),
                    Capability::new_static("urn:ietf:params:xml:ns:netconf:base:1.0?module=ietf-netconf&amp;revision=2011-06-01"),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:candidate:1.0",
                    ),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:confirmed-commit:1.0",
                    ),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:validate:1.0",
                    ),
                    Capability::new_static(
                        "urn:ietf:params:xml:ns:netconf:capability:url:1.0?scheme=http,ftp,file",
                    ),
                    Capability::new_static("urn:ietf:params:xml:ns:yang:ietf-inet-types?module=ietf-inet-types&amp;revision=2013-07-15"),
                    Capability::new_static("urn:ietf:params:xml:ns:yang:ietf-yang-metadata?module=ietf-yang-metadata&amp;revision=2016-08-05"),
                    Capability::new_static("urn:ietf:params:xml:ns:yang:ietf-netconf-monitoring"),
                    Capability::new_static("http://xml.juniper.net/netconf/junos/1.0"),
                    Capability::new_static("http://xml.juniper.net/dmi/system/1.0"),
                    Capability::new_static("http://yang.juniper.net/junos/jcmd?module=junos-configuration-metadata&amp;revision=2021-09-01")
                ]
                .into(),
            },
            session_id: 43129,
        };
        assert_eq!(expect, ServerHello::from_xml(xml).unwrap());
    }

    #[test]
    fn client_hello_to_xml() {
        let req = ClientHello {
            capabilities: Capabilities {
                inner: [Capability::new_static("urn:ietf:params:netconf:base:1.0")].into(),
            },
        };
        let expect = "<hello><capabilities><capability>urn:ietf:params:netconf:base:1.0</capability></capabilities></hello>]]>]]>";
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
