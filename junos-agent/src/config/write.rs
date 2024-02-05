use std::{fmt::Debug, io::Write};

use ip::{
    any::PrefixSet,
    concrete,
    traits::{Afi, PrefixSet as _},
    Ipv4, Ipv6,
};

use netconf::message::{WriteError, WriteXml};

use quick_xml::{events::BytesText, Writer};

use super::{EvaluatedPolicyStmts, PolicyStmt};

pub(crate) trait WriteConfig: WriteXml + Debug + Send + Sync {}

impl WriteConfig for EvaluatedPolicyStmts {}

impl WriteXml for EvaluatedPolicyStmts {
    #[tracing::instrument(skip(self, writer), level = "debug")]
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element("configuration")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("policy-options")
                    .write_inner_content(|writer| {
                        self.0.iter().try_for_each(|stmt| stmt.write_xml(writer))
                    })?;
                Ok(())
            })
            .map(|_| ())
    }
}

impl WriteXml for PolicyStmt<PrefixSet> {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element("policy-statement")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("name")
                    .write_text_content(BytesText::new(&self.name))?;
                let (ipv4, ipv6) = self.partition();
                if ipv4.is_non_empty() {
                    ipv4.write_xml(writer)?;
                }
                if ipv6.is_non_empty() {
                    ipv6.write_xml(writer)?;
                }
                _ = writer
                    .create_element("then")
                    .write_inner_content(|writer| {
                        writer.create_element("reject").write_empty().map(|_| ())
                    })?;
                Ok(())
            })
            .map(|_| ())
    }
}

impl PolicyStmt<PrefixSet> {
    const fn partition(&self) -> (Term<'_, Ipv4>, Term<'_, Ipv6>) {
        let (ipv4, ipv6) = self.content.as_partitions();
        (Term(ipv4), Term(ipv6))
    }
}

struct Term<'a, A: Afi>(&'a concrete::PrefixSet<A>);

impl<A: Afi> Term<'_, A> {
    fn is_non_empty(&self) -> bool {
        !self.0.is_empty()
    }
}

impl<A: Afi> WriteXml for Term<'_, A> {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let afi_name = match A::as_afi() {
            ip::concrete::Afi::Ipv4 => "inet",
            ip::concrete::Afi::Ipv6 => "inet6",
        };
        writer
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
                        self.0.ranges().try_for_each(|range| {
                            _ = writer.create_element("route-filter").write_inner_content(
                                |writer| {
                                    _ = writer.create_element("address").write_text_content(
                                        BytesText::new(&range.prefix().to_string()),
                                    )?;
                                    _ = writer
                                        .create_element("prefix-length-range")
                                        .write_text_content(BytesText::new(&format!(
                                            "/{}-/{}",
                                            range.lower(),
                                            range.upper()
                                        )))?;
                                    Ok::<_, WriteError>(())
                                },
                            )?;
                            Ok::<_, WriteError>(())
                        })?;
                        Ok::<_, WriteError>(())
                    })?;
                _ = writer
                    .create_element("then")
                    .write_inner_content(|writer| {
                        writer.create_element("accept").write_empty().map(|_| ())
                    })?;
                Ok(())
            })
            .map(|_| ())
    }
}
