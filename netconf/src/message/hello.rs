use std::{
    borrow::{Borrow, Cow},
    collections::HashSet,
    fmt::Debug,
};

use serde::{Deserialize, Serialize};

use crate::Error;

use super::{ClientMsg, FromXml, ServerMsg, ToXml};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ServerHello {
    capabilities: Capabilities,
    session_id: usize,
}

impl ServerHello {
    pub(crate) const fn session_id(&self) -> usize {
        self.session_id
    }
}

impl FromXml for ServerHello {
    type Error = Error;

    fn from_xml<S>(input: S) -> Result<Self, Self::Error>
    where
        S: AsRef<str> + Debug,
    {
        Ok(quick_xml::de::from_str(input.as_ref())?)
    }
}

impl ServerMsg for ServerHello {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct ClientHello {
    capabilities: Capabilities,
}

impl ClientHello {
    pub(crate) fn new(capabilities: &[Capability]) -> Self {
        Self {
            capabilities: capabilities.iter().cloned().collect(),
        }
    }

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

impl ToXml for ClientHello {
    type Error = Error;

    fn to_xml(&self) -> Result<String, Self::Error> {
        Ok(quick_xml::se::to_string_with_root("hello", self)?)
    }
}

impl ClientMsg for ClientHello {
    // const ROOT_TAG: &'static str = "hello";
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Capabilities {
    #[serde(rename = "capability")]
    inner: HashSet<Capability>,
}

impl Capabilities {
    pub(crate) fn iter(&self) -> impl Iterator<Item = &Capability> {
        self.inner.iter()
    }

    pub(crate) fn contains(&self, elem: &Capability) -> bool {
        self.inner.contains(elem)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    #[serde(rename = "$value")]
    uri: Cow<'static, str>,
}

impl Capability {
    #[must_use]
    pub const fn new(uri: String) -> Self {
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

pub(crate) const BASE: Capability = Capability::new_static("urn:ietf:params:netconf:base:1.0");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_server_hello() {
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
        assert_eq!(expect, quick_xml::de::from_str(xml).unwrap());
    }
}
