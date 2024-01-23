use std::{fmt::Debug, io::Write};

use quick_xml::{
    events::{BytesStart, Event},
    name::{Namespace, ResolveResult},
    NsReader, Writer,
};

use crate::{
    capabilities::{Capabilities, Capability},
    session::SessionId,
};

use super::{xmlns, ClientMsg, ReadError, ReadXml, ServerMsg, WriteError, WriteXml};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServerHello {
    capabilities: Capabilities,
    session_id: SessionId,
}

impl ServerHello {
    pub(crate) const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub(crate) fn capabilities(self) -> Capabilities {
        self.capabilities
    }
}

impl ReadXml for ServerHello {
    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
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
                    return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                }
            };
        }
        Ok(Self {
            capabilities: capabilities
                .ok_or_else(|| ReadError::missing_element("hello", "capabilities"))?,
            session_id: session_id
                .ok_or_else(|| ReadError::missing_element("hello", "session-id"))?,
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

    #[tracing::instrument(level = "debug")]
    pub(crate) fn capabilities(self) -> Capabilities {
        self.capabilities
    }
}

impl WriteXml for ClientHello {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        _ = Writer::new(writer)
            .create_element("hello")
            .write_inner_content(|writer| self.capabilities.write_xml(writer.get_mut()))?;
        Ok(())
    }
}

impl ClientMsg for ClientHello {}

#[cfg(test)]
mod tests {
    use iri_string::types::UriStr;

    use crate::capabilities::Base;

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
            capabilities: [
                Capability::Base(Base::V1_0),
                Capability::Candidate,
                Capability::ConfirmedCommitV1_0,
                Capability::ValidateV1_0,
                Capability::Url(vec!["http".into(), "ftp".into(), "file".into()]),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:netconf:base:1.0")
                        .unwrap()
                        .into(),
                ),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:netconf:capability:candidate:1.0")
                        .unwrap()
                        .into(),
                ),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:netconf:capability:confirmed-commit:1.0")
                        .unwrap()
                        .into(),
                ),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:netconf:capability:validate:1.0")
                        .unwrap()
                        .into(),
                ),
                Capability::Unknown(
                    UriStr::new(
                        "urn:ietf:params:xml:ns:netconf:capability:url:1.0?scheme=http,ftp,file",
                    )
                    .unwrap()
                    .into(),
                ),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:yang:ietf-netconf-monitoring")
                        .unwrap()
                        .into(),
                ),
                #[cfg(feature = "junos")]
                Capability::JunosXmlManagementProtocol,
                #[cfg(not(feature = "junos"))]
                Capability::Unknown(
                    UriStr::new("http://xml.juniper.net/netconf/junos/1.0")
                        .unwrap()
                        .into(),
                ),
                Capability::Unknown(
                    UriStr::new("http://xml.juniper.net/dmi/system/1.0")
                        .unwrap()
                        .into(),
                ),
            ]
            .into_iter()
            .collect(),
            session_id: SessionId::new(802).unwrap(),
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
            capabilities: [
                Capability::Base(Base::V1_0),
                Capability::Candidate,
                Capability::ConfirmedCommitV1_0,
                Capability::ValidateV1_0,
                Capability::Url(vec!["http".into(), "ftp".into(), "file".into()]),
                Capability::Unknown(UriStr::new("urn:ietf:params:xml:ns:netconf:base:1.0?module=ietf-netconf&amp;revision=2011-06-01").unwrap().into()),
                Capability::Unknown(UriStr::new("urn:ietf:params:xml:ns:netconf:capability:candidate:1.0").unwrap().into()),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:netconf:capability:confirmed-commit:1.0").unwrap().into(),
                ),
                Capability::Unknown(UriStr::new("urn:ietf:params:xml:ns:netconf:capability:validate:1.0").unwrap().into()),
                Capability::Unknown(
                    UriStr::new("urn:ietf:params:xml:ns:netconf:capability:url:1.0?scheme=http,ftp,file").unwrap().into(),
                ),
                Capability::Unknown(UriStr::new("urn:ietf:params:xml:ns:yang:ietf-inet-types?module=ietf-inet-types&amp;revision=2013-07-15").unwrap().into()),
                Capability::Unknown(UriStr::new("urn:ietf:params:xml:ns:yang:ietf-yang-metadata?module=ietf-yang-metadata&amp;revision=2016-08-05").unwrap().into()),
                Capability::Unknown(UriStr::new("urn:ietf:params:xml:ns:yang:ietf-netconf-monitoring").unwrap().into()),
                #[cfg(feature = "junos")]
                Capability::JunosXmlManagementProtocol,
                #[cfg(not(feature = "junos"))]
                Capability::Unknown(UriStr::new("http://xml.juniper.net/netconf/junos/1.0").unwrap().into()),
                Capability::Unknown(UriStr::new("http://xml.juniper.net/dmi/system/1.0").unwrap().into()),
                Capability::Unknown(UriStr::new("http://yang.juniper.net/junos/jcmd?module=junos-configuration-metadata&amp;revision=2021-09-01").unwrap().into()),
            ]
            .into_iter()
            .collect(),
            session_id: SessionId::new(43129).unwrap(),
        };
        assert_eq!(expect, ServerHello::from_xml(xml).unwrap());
    }

    #[test]
    fn client_hello_to_xml() {
        let req = ClientHello {
            capabilities: std::iter::once(Capability::Base(Base::V1_0)).collect(),
        };
        let expect = "<hello><capabilities><capability>urn:ietf:params:netconf:base:1.0</capability></capabilities></hello>]]>]]>";
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
