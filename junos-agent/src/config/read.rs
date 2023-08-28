use xmhell::{quick_xml::Reader, Expect};

use super::{CandidatePolicyStmts, Empty, PolicyStmt};

pub(crate) trait FromXml: Sized {
    const QUERY_CMD: &'static str;
    fn from_xml(input: &str) -> anyhow::Result<Self>;
}

impl FromXml for CandidatePolicyStmts {
    const QUERY_CMD: &'static str = r#"
        <get-config>
            <source>
                <running/>
            </source>
            <filter type="subtree">
                <configuration>
                    <policy-options>
                        <policy-statement/>
                    </policy-options>
                </configuration>
            </filter>
        </get-config>
    "#;

    #[allow(clippy::too_many_lines)]
    fn from_xml(input: &str) -> anyhow::Result<Self> {
        let mut reader = Reader::from_str(input);
        _ = reader.trim_text(true);
        let results = reader.expect_element("data")?.read_inner(|reader| {
            Ok(reader
                .expect_element("configuration")?
                .read_inner(|reader| {
                    Ok(reader
                        .expect_element("policy-options")?
                        .read_inner(|reader| {
                            let mut buf = Vec::new();
                            loop {
                                match reader.expect_element("junos:comment") {
                                    Ok(inner) => {
                                        let Ok(filter_expr) = inner.read_inner(|reader| {
                                            Ok(reader
                                                .expect_text()?
                                                .trim_matches(['/', '*'].as_slice())
                                                .trim()
                                                .strip_prefix("bgpfu-fltr:")
                                                .ok_or_else(|| anyhow::anyhow!("no match"))?
                                                .parse()?)
                                        }) else {
                                            continue;
                                        };
                                        let Ok(name) = reader
                                            .expect_element("policy-statement")
                                            .and_then(|inner| {
                                                inner.read_inner(|reader| {
                                                    let name = reader
                                                        .expect_element("name")?
                                                        .read_inner(|reader| {
                                                            Ok(reader.expect_text()?.into_owned())
                                                        })?;
                                                    reader.expect_element("then")?.read_inner(
                                                        |reader| Ok(reader.expect_empty("reject")?),
                                                    )?;
                                                    Ok(name)
                                                })
                                            })
                                        else {
                                            continue;
                                        };
                                        let candidate = PolicyStmt {
                                            filter_expr,
                                            name,
                                            content: Empty,
                                        };
                                        log::info!("found candidate policy {candidate:?}");
                                        buf.push(candidate);
                                    }
                                    Err(xmhell::Error::Eof) => break,
                                    Err(xmhell::Error::UnexpectedEvent(_)) => continue,
                                    Err(err) => return Err(err.into()),
                                }
                            }
                            Ok(buf)
                        })?)
                })?)
        })?;
        reader.expect_eof()?;
        Ok(Self(results))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_from_xml {
        ( $( $( #[$attr:meta] )* $name:ident { $input:literal => $result:expr } )* ) => {
            $(
                #[test]
                $( #[$attr] )*
                fn $name() {
                    assert_eq!(CandidatePolicyStmts::from_xml($input).unwrap(), $result);
                }
            )*
        }
    }

    test_from_xml! {
        #[should_panic]
        no_root {
            "" => CandidatePolicyStmts::default()
        }
        #[should_panic]
        bad_root {
            "<foo></foo>" => CandidatePolicyStmts::default()
        }
        #[should_panic]
        mismatched_root {
            "<data></atad>" => CandidatePolicyStmts::default()
        }
        #[should_panic]
        empty_data {
            "<data></data>" => CandidatePolicyStmts::default()
        }
        #[should_panic]
        empty_configuration {
            r#"
                <data>
                    <configuration></configuration>
                </data>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic]
        premature_eof_in_data {
            r#"
                <data>
                    <configuration>
                        <policy-options></policy-options>
                    </configuration>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic]
        premature_eof_in_configuration {
            r#"
                <data>
                    <configuration>
                        <policy-options></policy-options>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic]
        premature_eof_in_policy_options {
            r#"
                <data>
                    <configuration>
                        <policy-options>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic]
        trailing_input {
            r#"
                <data>
                    <configuration foo="bar">
                        <policy-options></policy-options>
                    </configuration>
                </data>
                <foo/>
            "# => CandidatePolicyStmts::default()
        }
        empty {
            r#"
                <data>
                    <configuration>
                        <policy-options></policy-options>
                    </configuration>
                </data>
            "# => CandidatePolicyStmts::default()
        }
        singleton {
            r#"
                <data>
                    <configuration>
                        <policy-options>
                            <junos:comment>/* bgpfu-fltr:AS-FOO */</junos:comment>
                            <policy-statement>
                                <name>fltr-foo</name>
                                <then><reject/></then>
                            </policy-statement>
                        </policy-options>
                    </configuration>
                </data>
            "# => CandidatePolicyStmts(vec![
                PolicyStmt { filter_expr: "AS-FOO".parse().unwrap(), name: "fltr-foo".to_string(), content: Empty }
            ])
        }
        complex {
            r#"
                <data>
                    <configuration>
                        <policy-options>

                            <other-tag/>

                            <junos:comment>/* bgpfu-fltr:AS-BAR OR AS65000 */</junos:comment>
                            <policy-statement>
                                <name>fltr-bar</name>
                                <then><reject/></then>
                            </policy-statement>

                            <junos:comment>/* unrelated comment */</junos:comment>
                            <policy-statement>
                                <name>unmanaged</name>
                            </policy-statement>

                            <junos:comment>/* bgpfu-fltr:AS-BAZ AND { 10.0.0.0/8 }^+ */</junos:comment>
                            <policy-statement>
                                <name>fltr-baz</name>
                                <then><reject/></then>
                            </policy-statement>

                            <junos:comment>/* bgpfu-fltr:error! */</junos:comment>
                            <policy-statement>
                                <name>malformed-filter-expr</name>
                            </policy-statement>

                        </policy-options>
                    </configuration>
                </data>
            "# => CandidatePolicyStmts(vec![
                PolicyStmt { filter_expr: "AS-BAR OR AS65000".parse().unwrap(), name: "fltr-bar".to_string(), content: Empty },
                PolicyStmt { filter_expr: "AS-BAZ AND { 10.0.0.0/8 }^+".parse().unwrap(), name: "fltr-baz".to_string(), content: Empty }
            ])
        }
    }
}
