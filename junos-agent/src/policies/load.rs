use std::{fmt::Debug, io::Write};

use chrono::{DateTime, Utc};
use ip::{Afi, PrefixRange};
use netconf::message::{WriteError, WriteXml};
use quick_xml::{events::BytesText, ElementWriter, Writer};

use super::{Differences, Update, Updates};

pub(crate) trait Load: Debug + Send + Sync {
    type Update: WriteXml + Debug + Send + Sync;

    fn updates(self) -> impl Iterator<Item = Self::Update> + Send + Sync;
}

impl<'a> Load for Updates<'a> {
    type Update = Update<'a>;

    fn updates(self) -> impl Iterator<Item = Self::Update> {
        self.inner.into_iter()
    }
}

impl Update<'_> {
    fn name(&self) -> BytesText<'_> {
        let name = match self {
            Self::Delete { name } | Self::Update { name, .. } => name,
        };
        BytesText::new(name.as_ref())
    }

    fn policy_stmt_elem<'a, W: Write>(&self, writer: &'a mut Writer<W>) -> ElementWriter<'a, W> {
        let elem = writer.create_element("policy-statement");
        match self {
            Self::Delete { .. } => elem.with_attribute(("delete", "delete")),
            Self::Update { filter_expr, .. } => {
                let now = if cfg!(test) {
                    DateTime::UNIX_EPOCH
                } else {
                    Utc::now()
                }
                .format("%Y-%m-%d %H:%M:%SZ");
                // TODO: do comments even work in the ephemeral database?
                elem.with_attribute((
                    "junos:comment",
                    format!("Last updated at {now} from mp-filter expression {filter_expr}")
                        .as_str(),
                ))
            }
        }
    }
}

impl WriteXml for Update<'_> {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element("configuration")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("policy-options")
                    .write_inner_content(|writer| {
                        self.policy_stmt_elem(writer)
                            .write_inner_content(|writer| {
                                _ = writer
                                    .create_element("name")
                                    .write_text_content(self.name())?;
                                match self {
                                    Self::Delete { .. } => Ok::<_, WriteError>(()),
                                    Self::Update { ipv4, ipv6, .. } => {
                                        ipv4.write_xml(writer)?;
                                        ipv6.write_xml(writer)?;
                                        _ = writer.create_element("then").write_inner_content(
                                            |writer| {
                                                writer
                                                    .create_element("reject")
                                                    .write_empty()
                                                    .map(|_| ())
                                            },
                                        )?;
                                        Ok(())
                                    }
                                }
                            })
                            .map(|_| ())
                    })?;
                Ok(())
            })
            .map(|_| ())
    }
}

impl<A: Afi> WriteXml for Differences<'_, A> {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let elem = {
            let elem = writer.create_element("term");
            match (self.old, self.new.is_empty()) {
                (Some(old), true) if !old.is_empty() => elem.with_attribute(("delete", "delete")),
                _ => elem,
            }
        };
        _ = elem.write_inner_content(|writer| {
            _ = writer
                .create_element("name")
                .write_text_content(afi_name::<A>())?;
            if !self.new.is_empty() {
                _ = writer
                    .create_element("from")
                    .write_inner_content(|writer| {
                        _ = writer
                            .create_element("family")
                            .write_text_content(afi_name::<A>())?;
                        match self.old {
                            None => self.new.iter().try_for_each(|range| {
                                write_route_filter::<_, A>(writer, range, false)
                            })?,
                            Some(old) => {
                                old.diff(self.new).try_for_each(|range| {
                                    write_route_filter::<_, A>(writer, range, true)
                                })?;
                                self.new.diff(old).try_for_each(|range| {
                                    write_route_filter::<_, A>(writer, range, false)
                                })?;
                            }
                        };
                        Ok::<_, WriteError>(())
                    })?;
                _ = writer
                    .create_element("then")
                    .write_inner_content(|writer| {
                        writer.create_element("accept").write_empty().map(|_| ())
                    })?;
            };
            Ok::<_, WriteError>(())
        })?;
        Ok(())
    }
}

fn afi_name<A: Afi>() -> BytesText<'static> {
    use ip::concrete::Afi::{Ipv4, Ipv6};
    let name = match A::as_afi() {
        Ipv4 => "inet",
        Ipv6 => "inet6",
    };
    BytesText::new(name)
}

fn write_route_filter<W: Write, A: Afi>(
    writer: &mut Writer<W>,
    range: &PrefixRange<A>,
    delete: bool,
) -> Result<(), WriteError> {
    let mut elem = writer.create_element("route-filter");
    if delete {
        elem = elem.with_attribute(("delete", "delete"));
    }
    elem.write_inner_content(|writer| {
        _ = writer
            .create_element("address")
            .write_text_content(BytesText::new(&range.prefix().to_string()))?;
        _ = writer
            .create_element("prefix-length-range")
            .write_text_content(BytesText::new(&format!(
                "/{}-/{}",
                range.lower(),
                range.upper()
            )))?;
        Ok::<_, WriteError>(())
    })
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::super::{Name, Ranges};
    use super::*;

    use std::iter::once;
    use std::str::from_utf8;

    macro_rules! assert_xml_written {
        ($update:expr, $expected:expr) => {
            let written = {
                let mut buf = Vec::new();
                let mut writer = Writer::new(&mut buf);
                $update.write_xml(&mut writer).unwrap();
                from_utf8(&buf).unwrap().to_string()
            };
            let expect: String = $expected.split('\n').map(str::trim_start).collect();
            assert_eq!(written, expect);
        };
    }

    #[test]
    fn delete_policy() {
        let update = Update::Delete {
            name: Name::new("fltr-foo"),
        };
        let expect = r#"
            <configuration>
                <policy-options>
                    <policy-statement delete="delete">
                        <name>fltr-foo</name>
                    </policy-statement>
                </policy-options>
            </configuration>"#;
        assert_xml_written!(update, expect);
    }

    #[test]
    fn delete_term() {
        let filter_expr = "{ 192.0.2.0/24^+, 2001:db8::/32^48-64}".parse().unwrap();
        let ipv4 = "192.0.2.0/24,24,32".parse().map(once).unwrap().collect();
        let ipv6 = "2001:db8::/32,48,64".parse().map(once).unwrap().collect();
        let update = Update::Update {
            name: Name::new("fltr-foo"),
            filter_expr: &filter_expr,
            ipv4: Differences {
                old: Some(&ipv4),
                new: &Ranges::default(),
            },
            ipv6: Differences {
                old: Some(&ipv6),
                new: &ipv6,
            },
        };
        let expect = r#"
            <configuration>
                <policy-options>
                    <policy-statement 
                        junos:comment="Last updated at 1970-01-01 00:00:00Z 
                                       from mp-filter expression {192.0.2.0/24^+, 2001:db8::/32^48-64}">
                        <name>fltr-foo</name>
                        <term delete="delete">
                            <name>inet</name>
                        </term>
                        <term>
                            <name>inet6</name>
                            <from>
                                <family>inet6</family>
                            </from>
                            <then><accept/></then>
                        </term>
                        <then><reject/></then>
                    </policy-statement>
                </policy-options>
            </configuration>"#;
        assert_xml_written!(update, expect);
    }

    #[test]
    fn update_term() {
        let ipv4_old = "192.0.2.0/24,24,32".parse().map(once).unwrap().collect();
        let ipv6_old = "2001:db8::/32,48,64".parse().map(once).unwrap().collect();
        let filter_expr = "{ 192.0.2.0/24^-, 2001:db8::/32^+ }".parse().unwrap();
        let ipv4_new = "192.0.2.0/24,25,32".parse().map(once).unwrap().collect();
        let ipv6_new = "2001:db8::/32,32,64".parse().map(once).unwrap().collect();
        let update = Update::Update {
            name: Name::new("fltr-foo"),
            filter_expr: &filter_expr,
            ipv4: Differences {
                old: Some(&ipv4_old),
                new: &ipv4_new,
            },
            ipv6: Differences {
                old: Some(&ipv6_old),
                new: &ipv6_new,
            },
        };
        let expect = r#"
            <configuration>
                <policy-options>
                    <policy-statement 
                        junos:comment="Last updated at 1970-01-01 00:00:00Z 
                                       from mp-filter expression {192.0.2.0/24^-, 2001:db8::/32^+}">
                        <name>fltr-foo</name>
                        <term>
                            <name>inet</name>
                            <from>
                                <family>inet</family>
                                <route-filter delete="delete">
                                    <address>192.0.2.0/24</address>
                                    <prefix-length-range>/24-/32</prefix-length-range>
                                </route-filter>
                                <route-filter>
                                    <address>192.0.2.0/24</address>
                                    <prefix-length-range>/25-/32</prefix-length-range>
                                </route-filter>
                            </from>
                            <then><accept/></then>
                        </term>
                        <term>
                            <name>inet6</name>
                            <from>
                                <family>inet6</family>
                                <route-filter delete="delete">
                                    <address>2001:db8::/32</address>
                                    <prefix-length-range>/48-/64</prefix-length-range>
                                </route-filter>
                                <route-filter>
                                    <address>2001:db8::/32</address>
                                    <prefix-length-range>/32-/64</prefix-length-range>
                                </route-filter>
                            </from>
                            <then><accept/></then>
                        </term>
                        <then><reject/></then>
                    </policy-statement>
                </policy-options>
            </configuration>"#;
        assert_xml_written!(update, expect);
    }

    #[test]
    fn create_policy() {
        let filter_expr = "{ 192.0.2.0/24^+, 2001:db8::/32^48-64 }".parse().unwrap();
        let ipv4_new = "192.0.2.0/24,24,32".parse().map(once).unwrap().collect();
        let ipv6_new = "2001:db8::/32,48,64".parse().map(once).unwrap().collect();
        let update = Update::Update {
            name: Name::new("fltr-foo"),
            filter_expr: &filter_expr,
            ipv4: Differences {
                old: None,
                new: &ipv4_new,
            },
            ipv6: Differences {
                old: None,
                new: &ipv6_new,
            },
        };
        let expect = r#"
            <configuration>
                <policy-options>
                    <policy-statement 
                        junos:comment="Last updated at 1970-01-01 00:00:00Z 
                                       from mp-filter expression {192.0.2.0/24^+, 2001:db8::/32^48-64}">
                        <name>fltr-foo</name>
                        <term>
                            <name>inet</name>
                            <from>
                                <family>inet</family>
                                <route-filter>
                                    <address>192.0.2.0/24</address>
                                    <prefix-length-range>/24-/32</prefix-length-range>
                                </route-filter>
                            </from>
                            <then><accept/></then>
                        </term>
                        <term>
                            <name>inet6</name>
                            <from>
                                <family>inet6</family>
                                <route-filter>
                                    <address>2001:db8::/32</address>
                                    <prefix-length-range>/48-/64</prefix-length-range>
                                </route-filter>
                            </from>
                            <then><accept/></then>
                        </term>
                        <then><reject/></then>
                    </policy-statement>
                </policy-options>
            </configuration>"#;
        assert_xml_written!(update, expect);
    }
}
