use std::io::Cursor;

use anyhow::Context;

use ip::traits::PrefixSet as _;

use xmhell::quick_xml::{self, events::BytesText, Writer};

use super::EvaluatedPolicyStmts;

pub(crate) trait ToXml {
    fn to_xml(self) -> anyhow::Result<String>;
}

impl ToXml for EvaluatedPolicyStmts {
    fn to_xml(self) -> anyhow::Result<String> {
        fn write_filter_term<A: ip::Afi, W: std::io::Write>(
            set: &ip::concrete::PrefixSet<A>,
            writer: &mut Writer<W>,
        ) -> Result<(), quick_xml::Error> {
            let afi_name = match A::as_afi() {
                ip::concrete::Afi::Ipv4 => "inet",
                ip::concrete::Afi::Ipv6 => "inet6",
            };
            _ = writer
                .create_element("term")
                .write_inner_content(|writer| {
                    _ = writer
                        .create_element("name")
                        .write_text_content(BytesText::new(afi_name))?;
                    _ = writer
                        .create_element("from")
                        .write_inner_content(|writer| {
                            _ = writer
                                .create_element("family")
                                .write_text_content(BytesText::new(afi_name))?;
                            set.ranges().try_for_each(|range| {
                                _ = writer.create_element("route-filter").write_inner_content(
                                    |writer| {
                                        _ = writer.create_element("address").write_text_content(
                                            BytesText::new(&format!("{}", range.prefix())),
                                        )?;
                                        _ = writer
                                            .create_element("prefix-length-range")
                                            .write_text_content(BytesText::new(&format!(
                                                "/{}-/{}",
                                                range.lower(),
                                                range.upper()
                                            )))?;
                                        Ok(())
                                    },
                                )?;
                                Ok::<_, quick_xml::Error>(())
                            })?;
                            Ok(())
                        })?;
                    _ = writer
                        .create_element("then")
                        .write_inner_content(|writer| {
                            _ = writer.create_element("accept").write_empty()?;
                            Ok(())
                        })?;
                    Ok(())
                })?;
            Ok(())
        }
        let mut buf = Cursor::new(Vec::new());
        let mut writer = Writer::new_with_indent(&mut buf, b' ', 4);
        _ = writer
            .create_element("configuration")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("policy-options")
                    .write_inner_content(|writer| {
                        self.0.into_iter().try_for_each(|stmt| {
                            _ = writer
                                .create_element("policy-statement")
                                .with_attribute(("operation", "replace"))
                                .write_inner_content(|writer| {
                                    _ = writer
                                        .create_element("name")
                                        .write_text_content(BytesText::new(&stmt.name))?;
                                    let (ipv4, ipv6) = stmt.content.partition();
                                    if !ipv4.is_empty() {
                                        write_filter_term(&ipv4, writer)?;
                                    }
                                    if !ipv6.is_empty() {
                                        write_filter_term(&ipv6, writer)?;
                                    }
                                    _ = writer.create_element("then").write_inner_content(
                                        |writer| {
                                            _ = writer.create_element("reject").write_empty()?;
                                            Ok(())
                                        },
                                    )?;
                                    Ok(())
                                })?;
                            Ok::<_, quick_xml::Error>(())
                        })?;
                        Ok(())
                    })?;
                Ok(())
            })
            .context("failed to write xml config")?;
        String::from_utf8(buf.into_inner()).context("utf-8 encoding error")
    }
}
