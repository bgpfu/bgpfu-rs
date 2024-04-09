use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
};

use anyhow::{anyhow, Context};
use ip::{
    concrete::{Prefix, PrefixRange},
    traits::PrefixRange as _,
    Afi, Ipv4, Ipv6,
};
use netconf::message::{rpc::operation::Datastore, ReadError, ReadXml};
use quick_xml::{
    events::{BytesStart, Event},
    name::{Namespace, ResolveResult},
    NsReader,
};
use rpsl::expr::MpFilterExpr;

use super::{Candidate, Installed, Name, Policies, Ranges};

pub(crate) trait Fetch: ReadXml + Debug + Send + Sync + Sized {
    const DATASTORE: Datastore;
    const FILTER: Option<&'static str>;
}

impl Fetch for Policies<Candidate> {
    const DATASTORE: Datastore = Datastore::Running;
    const FILTER: Option<&'static str> = Some(
        r"
            <configuration>
                <policy-options>
                    <policy-statement/>
                </policy-options>
            </configuration>
        ",
    );
}

impl Fetch for Policies<Installed> {
    const DATASTORE: Datastore = Datastore::Candidate;
    const FILTER: Option<&'static str> = None;
}

const XNM: Namespace<'_> = Namespace(b"http://xml.juniper.net/xnm/1.1/xnm");
const JCMD: Namespace<'_> = Namespace(b"http://yang.juniper.net/junos/jcmd");

struct Maybe<T>(Option<(Name, T)>);

impl<T> ReadXml for Policies<T>
where
    Maybe<T>: ReadXml,
{
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "debug")]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut this = None;
        tracing::debug!("expecting <configuration>");
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"configuration" && this.is_none() =>
                {
                    tracing::debug!(?tag);
                    let end = tag.to_end();
                    let mut policy_options_seen = false;
                    let mut map = HashMap::new();
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(XNM), Event::Start(tag))
                                if tag.local_name().as_ref() == b"policy-options"
                                    && !policy_options_seen =>
                            {
                                tracing::debug!(?tag);
                                policy_options_seen = true;
                                let end = tag.to_end();
                                loop {
                                    match reader.read_resolved_event()? {
                                        (ResolveResult::Bound(XNM), Event::Start(tag))
                                            if tag.local_name().as_ref() == b"policy-statement" =>
                                        {
                                            tracing::debug!(?tag);
                                            if let Maybe(Some((name, policy))) =
                                                Maybe::read_xml(reader, &tag)?
                                            {
                                                if let Entry::Vacant(entry) =
                                                    map.entry(name.clone())
                                                {
                                                    _ = entry.insert(policy);
                                                } else {
                                                    let err = anyhow!("detected duplicate policy-statement '{name}'");
                                                    tracing::error!(%err);
                                                    return Err(ReadError::Other(err.into()));
                                                };
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
                            }
                            (_, Event::Comment(_)) => continue,
                            (_, Event::End(tag)) if tag == end => break,
                            (ns, event) => {
                                tracing::error!(?event, ?ns, "unexpected xml event");
                                return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                            }
                        }
                    }
                    this = Some(Self { map });
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        this.ok_or(ReadError::MissingElement {
            msg_type: "get-config rpc-reply",
            element: "configuration",
        })
    }
}

impl ReadXml for Maybe<Candidate> {
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "debug")]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut maybe_filter_expr = None;
        // TODO: **junos bug**
        // if both `active` and `comment` attributes are present on an object, then the JUNOS
        // NETCONF server emits duplicate `xmlns:jcmd` attributes.
        // duplicate attribute checks are disabled as a temporary workaround.
        // TODO:
        // avoid deleting previously installed policy-statement, if the running-config now has a
        // malformed mp-filter expr.
        for attr in start.attributes().with_checks(false) {
            let attr = attr.map_err(|err| ReadError::Other(err.into()))?;
            match reader.resolve_attribute(attr.key) {
                (ResolveResult::Bound(JCMD), name)
                    if name.as_ref() == b"active" && attr.unescape_value()? == "false" =>
                {
                    tracing::debug!("skipping inactive policy-statement");
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
                    name = Some(reader.read_text(tag.to_end().name()).map(Name::new)?);
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
            Ok(Self(Some((
                name.ok_or(ReadError::MissingElement {
                    msg_type: "policy-statement",
                    element: "name",
                })?,
                Candidate { filter_expr },
            ))))
        } else {
            tracing::warn!("skipping policy-statement '{name:?}' without trival reject action");
            Ok(Self(None))
        }
    }
}

impl ReadXml for Maybe<Installed> {
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "debug")]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut name = None;
        let (mut ipv4, mut ipv6) = (None, None);
        let mut default_reject = false;
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"name" && name.is_none() =>
                {
                    tracing::debug!(?tag);
                    name = Some(reader.read_text(tag.to_end().name()).map(Name::new)?);
                    tracing::debug!(?name);
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"term" =>
                {
                    tracing::debug!(?tag);
                    let term = Term::borrowed_read_xml(reader, &tag)?;
                    match term.from.family.as_ref() {
                        "inet" if ipv4.is_none() => {
                            ipv4 = Some(term.from.try_into_ranges::<Ipv4>()?);
                        }
                        "inet6" if ipv6.is_none() => {
                            ipv6 = Some(term.from.try_into_ranges::<Ipv6>()?);
                        }
                        family => {
                            return Err(ReadError::Other(
                                anyhow!("unexpected address family identifier '{family}'").into(),
                            ));
                        }
                    }
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"then" && !default_reject =>
                {
                    tracing::debug!(?tag);
                    let end = tag.to_end();
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(XNM), Event::Empty(tag))
                                if tag.local_name().as_ref() == b"reject" =>
                            {
                                default_reject = true;
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
        if default_reject {
            Ok(Self(Some((
                name.ok_or(ReadError::MissingElement {
                    msg_type: "policy-statement",
                    element: "name",
                })?,
                Installed {
                    ipv4: ipv4.unwrap_or_default(),
                    ipv6: ipv6.unwrap_or_default(),
                },
            ))))
        } else {
            tracing::warn!("skipping policy-statement '{name:?}' without default reject term");
            Ok(Self(None))
        }
    }
}

trait BorrowedReadXml<'i>: Sized + 'i {
    fn borrowed_read_xml(
        reader: &mut NsReader<&'i [u8]>,
        start: &BytesStart<'_>,
    ) -> Result<Self, ReadError>;
}

struct Term<'a> {
    // We don't currently need to read `name` after checking that it matches `family`, but we save
    // it just in case it is useful in future.
    #[allow(dead_code)]
    name: Cow<'a, str>,
    from: TermFrom<'a>,
}

impl<'i> BorrowedReadXml<'i> for Term<'i> {
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "trace")]
    fn borrowed_read_xml(
        reader: &mut NsReader<&'i [u8]>,
        start: &BytesStart<'_>,
    ) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut name = None;
        let mut from = None;
        let mut then = false;
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"name" && name.is_none() =>
                {
                    tracing::trace!(?tag);
                    name = Some(reader.read_text(tag.to_end().name())?);
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"from" && from.is_none() =>
                {
                    tracing::trace!(?tag);
                    from = Some(TermFrom::borrowed_read_xml(reader, &tag)?);
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"then" && !then =>
                {
                    tracing::trace!(?tag);
                    let end = tag.to_end();
                    let mut accept = false;
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(XNM), Event::Empty(tag))
                                if tag.local_name().as_ref() == b"accept" && !accept =>
                            {
                                tracing::trace!(?tag);
                                accept = true;
                            }
                            (_, Event::Comment(_)) => continue,
                            (_, Event::End(tag)) if tag == end => break,
                            (ns, event) => {
                                tracing::error!(?event, ?ns, "unexpected xml event");
                                return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                            }
                        }
                    }
                    if accept {
                        then = true;
                    } else {
                        return Err(ReadError::MissingElement {
                            msg_type: "then",
                            element: "accept",
                        });
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
        if then {
            let name = name.ok_or(ReadError::MissingElement {
                msg_type: "policy-statement",
                element: "name",
            })?;
            let from = from.ok_or(ReadError::MissingElement {
                msg_type: "policy-statement",
                element: "from",
            })?;
            if name != from.family {
                return Err(ReadError::Other(
                    anyhow!("mismatched term <name> and <family>").into(),
                ));
            }
            Ok(Self { name, from })
        } else {
            Err(ReadError::MissingElement {
                msg_type: "policy-statement",
                element: "then",
            })
        }
    }
}

struct TermFrom<'i> {
    family: Cow<'i, str>,
    route_filters: Vec<RouteFilter<'i>>,
}

impl TermFrom<'_> {
    fn try_into_ranges<A>(&self) -> Result<Ranges<A>, ReadError>
    where
        A: Afi,
    {
        match (A::as_afi(), self.family.as_ref()) {
            (ip::concrete::Afi::Ipv4, "inet") | (ip::concrete::Afi::Ipv6, "inet6") => {
                // No-op: Afi and address family name match
            }
            (afi, family) => {
                return Err(ReadError::Other(
                    anyhow!("can't parse '{family}' term into {afi} prefix-ranges",).into(),
                ))
            }
        };
        self.route_filters
            .iter()
            .map(|route_filter| {
                let base: PrefixRange<A> = route_filter.address.parse::<Prefix<_>>()?.into();
                let (lower, upper) = route_filter
                    .prefix_length_range
                    .split_once('-')
                    .ok_or_else(|| {
                        anyhow!(
                            "malformed prefix-length-range '{0}'",
                            route_filter.prefix_length_range
                        )
                    })
                    .and_then(|(l, u)| {
                        Ok((
                            l.parse().context("failed to parse lower range bound")?,
                            u.parse().context("failed to parse upper range bound")?,
                        ))
                    })?;
                base.with_length_range(lower..=upper)
                    .ok_or_else(|| anyhow!("invalid prefix range"))
            })
            .collect::<Result<_, _>>()
            .map_err(|err| ReadError::Other(err.into()))
            .map(|inner| Ranges { inner })
    }
}

impl<'i> BorrowedReadXml<'i> for TermFrom<'i> {
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "trace")]
    fn borrowed_read_xml(
        reader: &mut NsReader<&'i [u8]>,
        start: &BytesStart<'_>,
    ) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut family = None;
        let mut route_filters = Vec::new();
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"family" && family.is_none() =>
                {
                    tracing::trace!(?tag);
                    family = Some(reader.read_text(tag.to_end().name())?);
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"route-filter" =>
                {
                    tracing::trace!(?tag);
                    let route_filter = RouteFilter::borrowed_read_xml(reader, &tag)?;
                    route_filters.push(route_filter);
                }
                (_, Event::Comment(_)) => continue,
                (_, Event::End(tag)) if tag == end => break,
                (ns, event) => {
                    tracing::error!(?event, ?ns, "unexpected xml event");
                    return Err(ReadError::UnexpectedXmlEvent(event.into_owned()));
                }
            }
        }
        Ok(Self {
            family: family.ok_or(ReadError::MissingElement {
                msg_type: "from",
                element: "family",
            })?,
            route_filters,
        })
    }
}

struct RouteFilter<'i> {
    address: Cow<'i, str>,
    prefix_length_range: Cow<'i, str>,
}

impl<'i> BorrowedReadXml<'i> for RouteFilter<'i> {
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "trace")]
    fn borrowed_read_xml(
        reader: &mut NsReader<&'i [u8]>,
        start: &BytesStart<'_>,
    ) -> Result<Self, ReadError> {
        let end = start.to_end();
        let (mut address, mut prefix_length_range) = (None, None);
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"address" && address.is_none() =>
                {
                    tracing::trace!(?tag);
                    address = Some(reader.read_text(tag.to_end().name())?);
                }
                (ResolveResult::Bound(XNM), Event::Start(tag))
                    if tag.local_name().as_ref() == b"choice-ident"
                        && prefix_length_range.is_none() =>
                {
                    tracing::trace!(?tag);
                    let ident = reader.read_text(tag.to_end().name())?;
                    if ident.as_ref() != "prefix-length-range" {
                        return Err(ReadError::Other(
                            anyhow!("unexpected 'choice-ident' value '{ident}'").into(),
                        ));
                    }
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(XNM), Event::Start(tag))
                                if tag.local_name().as_ref() == b"choice-value" =>
                            {
                                tracing::trace!(?tag);
                                prefix_length_range = Some(reader.read_text(tag.to_end().name())?);
                                break;
                            }
                            (_, Event::Comment(_)) => continue,
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
        Ok(Self {
            address: address.ok_or(ReadError::MissingElement {
                msg_type: "route-filter",
                element: "address",
            })?,
            prefix_length_range: prefix_length_range.ok_or(ReadError::MissingElement {
                msg_type: "route-filter",
                element: "prefix-length-range",
            })?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use super::*;

    #[test]
    fn read_installed_policies() {
        let input = r#"
            <root
                xmlns="urn:ietf:params:xml:ns:netconf:base:1.0"
                xmlns:junos="http://xml.juniper.net/junos/23.1R0/junos">
                <configuration
                    xmlns="http://xml.juniper.net/xnm/1.1/xnm"
                    junos:changed-seconds="1709120869"
                    junos:changed-localtime="2024-02-28 11:47:49 UTC">
                    <policy-options>
                        <policy-statement>
                            <name>fltr-foo</name>
                            <term>
                                <name>inet</name>
                                <from>
                                    <family>inet</family>
                                    <route-filter>
                                        <address>197.157.64.0/19</address>
                                        <choice-ident>prefix-length-range</choice-ident>
                                        <choice-value>/20-/24</choice-value>
                                    </route-filter>
                                    <route-filter>
                                        <address>41.78.188.0/22</address>
                                        <choice-ident>prefix-length-range</choice-ident>
                                        <choice-value>/23-/24</choice-value>
                                    </route-filter>
                                </from>
                                <then><accept/></then>
                            </term>
                            <term>
                                <name>inet6</name>
                                <from>
                                    <family>inet6</family>
                                    <route-filter>
                                        <address>2c0f:fa90::/32</address>
                                        <choice-ident>prefix-length-range</choice-ident>
                                        <choice-value>/33-/48</choice-value>
                                    </route-filter>
                                </from>
                                <then><accept/></then>
                            </term>
                            <then><reject/></then>
                        </policy-statement>
                    </policy-options>
                </configuration>
            </root>
        "#;
        let expected = Policies {
            map: once((
                Name::new("fltr-foo"),
                Installed {
                    ipv4: [
                        "197.157.64.0/19,20,24".parse().unwrap(),
                        "41.78.188.0/22,23,24".parse().unwrap(),
                    ]
                    .into_iter()
                    .collect(),
                    ipv6: once("2c0f:fa90::/32,33,48".parse().unwrap()).collect(),
                },
            ))
            .collect(),
        };
        let mut reader = NsReader::from_str(input);
        let (_, Event::Start(start)) = reader.trim_text(true).read_resolved_event().unwrap() else {
            panic!("expected valid start tag");
        };
        let read = Policies::read_xml(&mut reader, &start).unwrap();
        assert_eq!(read, expected);
    }

    macro_rules! candidates_from_xml {
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
                                result = Some(Policies::<Candidate>::read_xml(&mut reader, &tag).unwrap());
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

    candidates_from_xml! {
        #[should_panic(expected = r#"MissingElement { msg_type: "get-config rpc-reply", element: "configuration" }"#)]
        no_root {
            "" => Policies::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Start(BytesStart { buf: Owned("foo"), name_len: 3 }))"#)]
        bad_root {
            "<foo></foo>" => Policies::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Start(BytesStart { buf: Owned("configuration"), name_len: 13 }))"#)]
        missing_xmlns {
            r"
                <configuration>
                    <policy-options></policy-options>
                </configuration>
            " => Policies::default()
        }
        #[should_panic(expected = r#"Xml(EndEventMismatch { expected: "configuration", found: "noitarugifnoc" })"#)]
        mismatched_root {
            r#"<configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm"></noitarugifnoc>"# => Policies::default()
        }
        #[should_panic(expected = r#"Xml(EndEventMismatch { expected: "configuration", found: "root" })"#)]
        premature_eof_in_configuration {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options></policy-options>
            "# => Policies::default()
        }
        #[should_panic(expected = r#"Xml(EndEventMismatch { expected: "policy-options", found: "root" })"#)]
        premature_eof_in_policy_options {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options>
            "# => Policies::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Empty(BytesStart { buf: Owned("foo"), name_len: 3 }))"#)]
        trailing_input {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm" foo="bar">
                    <policy-options></policy-options>
                </configuration>
                <foo/>
            "# => Policies::default()
        }
        #[should_panic(expected = r#"UnexpectedXmlEvent(Start(BytesStart { buf: Owned("policy-options"), name_len: 14 }))"#)]
        duplicate_policy_options {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm" foo="bar">
                    <policy-options></policy-options>
                    <policy-options></policy-options>
                </configuration>
            "# => Policies::default()
        }
        empty_configuration {
            r#"<configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm"></configuration>"# => Policies::default()
        }
        empty_policy_options {
            r#"
                <configuration xmlns="http://xml.juniper.net/xnm/1.1/xnm">
                    <policy-options></policy-options>
                </configuration>
            "# => Policies::default()
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
            "# => Policies {
                map: once((
                    Name::new("fltr-foo"),
                    Candidate {
                        filter_expr: "AS-FOO".parse().unwrap(),
                    }
                ))
                .collect()
            }
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
            "# => Policies {
                map: [
                    (
                        Name::new("fltr-bar"),
                        Candidate {
                            filter_expr: "AS-BAR OR AS65000".parse().unwrap(),
                        },
                    ),
                    (
                        Name::new("fltr-baz"),
                        Candidate {
                            filter_expr: "AS-BAZ AND { 10.0.0.0/8 }^+".parse().unwrap(),
                        },
                    ),
                ]
                .into_iter()
                .collect()
            }
        }
    }
}
