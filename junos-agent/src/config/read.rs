use std::fmt::Debug;

use netconf::message::{ReadError, ReadXml};
use quick_xml::{
    events::{BytesStart, Event},
    name::{Namespace, ResolveResult},
    NsReader,
};
use rpsl::expr::MpFilterExpr;

use super::{CandidatePolicyStmts, Empty, PolicyStmt};

pub(crate) trait ReadConfig: ReadXml + Debug + Send + Sync {
    const FILTER: &'static str;
}

impl ReadConfig for CandidatePolicyStmts {
    const FILTER: &'static str = r"
        <configuration>
            <policy-options>
                <policy-statement/>
            </policy-options>
        </configuration>
    ";
}

const XNM: Namespace<'_> = Namespace(b"http://xml.juniper.net/xnm/1.1/xnm");
const JCMD: Namespace<'_> = Namespace(b"http://yang.juniper.net/junos/jcmd");

struct MaybePolicyStmt(Option<PolicyStmt<Empty>>);

impl ReadXml for MaybePolicyStmt {
    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut maybe_filter_expr = None;
        // TODO: **junos bug**
        // if both `active` and `comment` attributes are present on an object, then the JUNOS
        // NETCONF server emits duplicate `xmlns:jcmd` attributes.
        // duplicate attribute checks are disabled as a temporary workaround.
        for attr in start.attributes().with_checks(false) {
            let attr = attr.map_err(|err| ReadError::Other(err.into()))?;
            match reader.resolve_attribute(attr.key) {
                (ResolveResult::Bound(JCMD), name)
                    if name.as_ref() == b"active" && attr.unescape_value()? == "false" =>
                {
                    _ = reader.read_to_end(end.name())?;
                    return Ok(Self(None));
                }
                (ResolveResult::Bound(JCMD), name) if name.as_ref() == b"comment" => {
                    let attr_value = attr.unescape_value()?;
                    let raw_expr = attr_value
                        .trim_matches(['/', '*'].as_slice())
                        .trim()
                        .strip_prefix("bgpfu-fltr:");
                    match raw_expr.map(|raw| (raw, raw.parse::<MpFilterExpr>())) {
                        Some((raw, Ok(expr))) => {
                            tracing::debug!(raw, ?expr);
                            maybe_filter_expr = Some(expr);
                        }
                        Some((raw, Err(err))) => {
                            tracing::warn!("skipping malformed filter expression '{raw}': {err}");
                        }
                        None => continue,
                    }
                }
                _ => continue,
            }
        }
        let Some(filter_expr) = maybe_filter_expr else {
            _ = reader.read_to_end(end.name())?;
            return Ok(Self(None));
        };
        let mut name = None;
        let mut reject_policy = false;
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"name" && name.is_none() =>
                {
                    name = Some(reader.read_text(tag.to_end().name())?);
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"then" && !reject_policy =>
                {
                    tracing::debug!(?tag);
                    let end = tag.to_end();
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(XNM), Event::Empty(tag))
                                if tag.local_name().as_ref() == b"reject" =>
                            {
                                reject_policy = true;
                            }
                            (_, Event::Comment(_)) => continue,
                            (_, Event::End(tag)) if tag == end => break,
                            (ns, event) => {
                                tracing::error!(?event, ?ns, "unexpected xml event");
                                return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                            }
                        }
                    }
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        if reject_policy {
            Ok(Self(Some(PolicyStmt {
                name: name
                    .ok_or(ReadError::MissingElement {
                        msg_type: "policy-statement",
                        element: "name",
                    })?
                    .to_string(),
                filter_expr,
                content: Empty,
            })))
        } else {
            tracing::warn!("skipping policy-statement '{name:?}' without trival reject action");
            Ok(Self(None))
        }
    }
}

impl ReadXml for CandidatePolicyStmts {
    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut outer = None;
        tracing::debug!("expecting <configuration>");
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"configuration" && outer.is_none() =>
                {
                    tracing::debug!(?tag);
                    let end = tag.to_end();
                    let mut inner = None;
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(XNM), Event::Start(tag))
                                if tag.local_name().as_ref() == b"policy-options"
                                    && inner.is_none() =>
                            {
                                tracing::debug!(?tag);
                                let end = tag.to_end();
                                let mut policies = Vec::new();
                                loop {
                                    match reader.read_resolved_event()? {
                                        (ResolveResult::Bound(XNM), Event::Start(tag))
                                            if tag.local_name().as_ref() == b"policy-statement" =>
                                        {
                                            tracing::debug!(?tag);
                                            if let MaybePolicyStmt(Some(policy)) =
                                                MaybePolicyStmt::read_xml(reader, &tag)?
                                            {
                                                policies.push(policy);
                                            }
                                        }
                                        (_, Event::Comment(_)) => continue,
                                        (_, Event::End(tag)) if tag == end => break,
                                        (ns, event) => {
                                            tracing::error!(?event, ?ns, "unexpected xml event");
                                            return Err(ReadError::UnexpectedXmlEvent(
                                                event.into_owned(),
                                            ));
                                        }
                                    }
                                }
                                inner = Some(Self(policies));
                            }
                            (_, Event::Comment(_)) => continue,
                            (_, Event::End(tag)) if tag == end => break,
                            (ns, event) => {
                                tracing::error!(?event, ?ns, "unexpected xml event");
                                return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                            }
                        }
                    }
                    outer = Some(inner.ok_or(ReadError::MissingElement {
                        msg_type: "get-config rpc-reply",
                        element: "policy-options",
                    })?);
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        outer.ok_or(ReadError::MissingElement {
            msg_type: "get-config rpc-reply",
            element: "configuration",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_from_xml {
        ( $( $( #[$attr:meta] )* $name:ident { $input:literal => $expect:expr } )* ) => {
            $(
                #[test]
                $( #[$attr] )*
                fn $name() {
                    let doc = format!("<root>{}</root>", $input);
                    let mut reader = NsReader::from_str(&doc);
                    _ = reader.trim_text(true);
                    let mut result = None;
                    loop {
                        match reader.read_resolved_event().unwrap() {
                            (ResolveResult::Unbound, Event::Start(tag)) if tag.local_name().as_ref() == b"root" => {
                                result = Some(CandidatePolicyStmts::read_xml(&mut reader, &tag).unwrap());
                            }
                            (_, Event::Eof) => break,
                            (ns, event) => panic!("unexpected xml event {event:?} ({ns:?})"),
                        }
                    }
                    assert_eq!(result.unwrap(), $expect);
                }
            )*
        }
    }

    test_from_xml! {
        #[should_panic(expected = r#"MissingElement { msg_type: "get-config rpc-reply", element: "configuration" }"#)]
        no_root {
            "" => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Start(BytesStart { buf: Owned("foo"), name_len: 3 }))"#)]
        bad_root {
            "<foo></foo>" => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Start(BytesStart { buf: Owned("configuration"), name_len: 13 }))"#)]
        missing_xmlns {
            r"
                <configuration>
                    <policy-options></policy-options>
                </configuration>
            " => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"Xml(EndEventMismatch { expected: "configuration", found: "noitarugifnoc" })"#)]
        mismatched_root {
            r#"<configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm"></noitarugifnoc>"# => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"MissingElement { msg_type: "get-config rpc-reply", element: "policy-options" }"#)]
        empty_configuration {
            r#"<configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm"></configuration>"# => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"Xml(EndEventMismatch { expected: "configuration", found: "root" })"#)]
        premature_eof_in_configuration {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options></policy-options>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"Xml(EndEventMismatch { expected: "policy-options", found: "root" })"#)]
        premature_eof_in_policy_options {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Empty(BytesStart { buf: Owned("foo"), name_len: 3 }))"#)]
        trailing_input {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm" foo="bar">
                    <policy-options></policy-options>
                </configuration>
                <foo/>
            "# => CandidatePolicyStmts::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Start(BytesStart { buf: Owned("policy-options"), name_len: 14 }))"#)]
        duplicate_policy_options {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm" foo="bar">
                    <policy-options></policy-options>
                    <policy-options></policy-options>
                </configuration>
            "# => CandidatePolicyStmts::default()
        }
        empty {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options></policy-options>
                </configuration>
            "# => CandidatePolicyStmts::default()
        }
        singleton {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options>
                        <policy-statement xmlns:jcmd="http://yang.juniper.net/junos/jcmd"
                                          jcmd:comment="/* bgpfu-fltr: AS-FOO */">
                            <name>fltr-foo</name>
                            <then><reject/></then>
                        </policy-statement>
                    </policy-options>
                </configuration>
            "# => CandidatePolicyStmts(vec![
                PolicyStmt { filter_expr: "AS-FOO".parse().unwrap(), name: "fltr-foo".to_string(), content: Empty }
            ])
        }
        complex {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options>

                        <policy-statement xmlns:jcmd="http://yang.juniper.net/junos/jcmd"
                                          jcmd:comment="/* bgpfu-fltr: AS-FOO */"
                                          jcmd:active="false">
                            <name>fltr-foo</name>
                            <then><reject/></then>
                        </policy-statement>

                        <policy-statement xmlns:jcmd="http://yang.juniper.net/junos/jcmd"
                                          jcmd:comment="/* bgpfu-fltr: AS-BAR OR AS65000 */">
                            <name>fltr-bar</name>
                            <then><reject/></then>
                        </policy-statement>

                        <policy-statement xmlns:jcmd="http://yang.juniper.net/junos/jcmd"
                                          jcmd:comment="/* unrelated comment */">
                            <name>unmanaged</name>
                        </policy-statement>

                        <policy-statement xmlns:jcmd="http://yang.juniper.net/junos/jcmd"
                                          jcmd:comment="/* bgpfu-fltr: AS-BAZ AND { 10.0.0.0/8 }^+ */">
                            <name>fltr-baz</name>
                            <then><reject/></then>
                        </policy-statement>

                        <policy-statement xmlns:jcmd="http://yang.juniper.net/junos/jcmd"
                                          jcmd:comment="/* bgpfu-fltr: error! */">
                            <name>malformed-filter-expr</name>
                        </policy-statement>

                    </policy-options>
                </configuration>
            "# => CandidatePolicyStmts(vec![
                PolicyStmt { filter_expr: "AS-BAR OR AS65000".parse().unwrap(), name: "fltr-bar".to_string(), content: Empty },
                PolicyStmt { filter_expr: "AS-BAZ AND { 10.0.0.0/8 }^+".parse().unwrap(), name: "fltr-baz".to_string(), content: Empty }
            ])
        }
    }
}
