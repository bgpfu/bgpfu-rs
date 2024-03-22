use std::{borrow::Cow, fmt::Debug, io::Write, sync::Arc};

use quick_xml::{
    events::{attributes::Attribute, BytesStart, Event},
    name::{QName, ResolveResult},
    ElementWriter, NsReader, Writer,
};

use crate::{
    capabilities::{Capability, Requirements},
    message::{
        rpc::{
            operation::{self, params::Required},
            IntoResult, Operation,
        },
        xmlns, ReadError, ReadXml, WriteError, WriteXml,
    },
    session::Context,
};

/// Request that the NETCONF server load configuration data into the candidate configuration or
/// open configuration database.
///
/// See [Juniper documentation][junos-docs].
///
/// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-load-configuration.html
#[derive(Debug, Clone)]
pub struct LoadConfiguration<S> {
    source: S,
}

impl<S> Operation for LoadConfiguration<S>
where
    S: Source + Debug + Send + Sync,
{
    const NAME: &'static str = "load-configuration";
    const REQUIRED_CAPABILITIES: Requirements =
        Requirements::One(Capability::JunosXmlManagementProtocol);
    type Builder<'a> = Builder<'a, S>;
    type Reply = Reply;
}

impl<S> WriteXml for LoadConfiguration<S>
where
    S: Source + Debug + Send + Sync,
{
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let elem = writer.create_element(Self::NAME);
        self.source.write_element(elem)
    }
}

trait AsAttribute {
    fn as_attribute(&self) -> impl Into<Attribute<'_>>;
}

trait Source {
    fn write_element<W: Write>(&self, elem: ElementWriter<'_, W>) -> Result<(), WriteError>;
}

#[derive(Debug, Clone)]
pub struct ConfigurationRevision(RevisionId);
impl AsAttribute for ConfigurationRevision {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("configuration-revision", self.0.as_ref())
    }
}
impl Source for ConfigurationRevision {
    fn write_element<W: Write>(&self, elem: ElementWriter<'_, W>) -> Result<(), WriteError> {
        _ = elem.with_attribute(self.as_attribute()).write_empty()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rescue;
impl AsAttribute for Rescue {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("rescue", "rescue")
    }
}
impl Source for Rescue {
    fn write_element<W: Write>(&self, elem: ElementWriter<'_, W>) -> Result<(), WriteError> {
        _ = elem.with_attribute(self.as_attribute()).write_empty()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rollback(RollbackIndex);
impl AsAttribute for Rollback {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        Attribute {
            key: QName(b"rollback"),
            value: Cow::Owned(self.0 .0.to_string().into()),
        }
    }
}
impl Source for Rollback {
    fn write_element<W: Write>(&self, elem: ElementWriter<'_, W>) -> Result<(), WriteError> {
        _ = elem.with_attribute(self.as_attribute()).write_empty()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Config<D, F, A> {
    data: D,
    format: F,
    action: A,
}

impl<D, F, A> Config<D, F, A> {
    pub const fn new(data: D, format: F, action: A) -> Self {
        Self {
            data,
            format,
            action,
        }
    }
}
impl<D, F, A> Source for Config<D, F, A>
where
    F: Format,
    A: Action<F>,
    D: ConfigData<F, A>,
{
    fn write_element<W: Write>(&self, elem: ElementWriter<'_, W>) -> Result<(), WriteError> {
        elem.with_attribute(self.format.as_attribute())
            .with_attribute(self.action.as_attribute())
            .write_inner_content(|writer| self.data.write_data(writer))
            .map(|_| ())
    }
}

// TODO: Do we need to check the :url capability before this can be used?
#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct Url<F, A> {
    url: operation::Url,
    format: F,
    action: A,
}
impl<F, A> AsAttribute for Url<F, A> {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("url", self.url.as_ref())
    }
}
impl<F, A> Source for Url<F, A>
where
    A: Action<F>,
    F: Format,
{
    fn write_element<W: Write>(&self, elem: ElementWriter<'_, W>) -> Result<(), WriteError> {
        _ = elem
            .with_attribute(self.as_attribute())
            .with_attribute(self.format.as_attribute())
            .with_attribute(self.action.as_attribute())
            .write_empty()?;
        Ok(())
    }
}

trait Format: AsAttribute {
    const DEFAULT_TAG: &'static str;
}

#[derive(Debug, Clone, Copy)]
pub struct Text;
impl Format for Text {
    const DEFAULT_TAG: &'static str = "configuration-text";
}
impl AsAttribute for Text {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("format", "text")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Xml;
impl Format for Xml {
    const DEFAULT_TAG: &'static str = "configuration";
}
impl AsAttribute for Xml {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("format", "xml")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Json;
impl Format for Json {
    const DEFAULT_TAG: &'static str = "configuration-json";
}
impl AsAttribute for Json {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("format", "json")
    }
}

trait ConfigData<F, A>
where
    F: Format,
    A: Action<F>,
{
    fn write_data<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError>;
}

impl<D, A> ConfigData<Text, A> for D
where
    D: AsRef<str>,
    A: Action<Text>,
{
    fn write_data<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(A::TAG)
            .write_inner_content(|writer| {
                writer
                    .get_mut()
                    .write_all(self.as_ref().as_bytes())
                    .map_err(|err| WriteError::Other(err.into()))
            })
            .map(|_| ())
    }
}

impl<D, A> ConfigData<Xml, A> for D
where
    D: WriteXml,
    A: Action<Xml>,
{
    fn write_data<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        self.write_xml(writer)
    }
}

impl<D, A> ConfigData<Json, A> for D
where
    // TODO: consider using `D: Serialize` instead
    D: AsRef<str>,
    A: Action<Json>,
{
    fn write_data<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(A::TAG)
            .write_inner_content(|writer| {
                writer
                    .get_mut()
                    .write_all(self.as_ref().as_bytes())
                    .map_err(|err| WriteError::Other(err.into()))
            })
            .map(|_| ())
    }
}

trait Action<F>: AsAttribute
where
    F: Format,
{
    const TAG: &'static str = F::DEFAULT_TAG;
}

#[derive(Debug, Clone, Copy)]
pub struct Merge;
impl Action<Xml> for Merge {}
impl Action<Text> for Merge {}
impl Action<Json> for Merge {}
impl AsAttribute for Merge {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("action", "merge")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Override;
impl Action<Xml> for Override {}
impl Action<Text> for Override {}
impl Action<Json> for Override {}
impl AsAttribute for Override {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("action", "override")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Update;
impl Action<Xml> for Update {}
impl Action<Text> for Update {}
impl Action<Json> for Update {}
impl AsAttribute for Update {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("action", "update")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Replace;
impl Action<Xml> for Replace {}
impl Action<Text> for Replace {}
impl AsAttribute for Replace {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("action", "replace")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Set;
impl Action<Text> for Set {
    const TAG: &'static str = "configuration-set";
}
impl AsAttribute for Set {
    fn as_attribute(&self) -> impl Into<Attribute<'_>> {
        ("action", "set")
    }
}

#[derive(Debug, Clone)]
// TODO: is `revision-id` a string?!
pub struct RevisionId(Arc<str>);

impl AsRef<str> for RevisionId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Debug, Clone, Copy)]
// TODO:
// - constrain to <=49
// - better API
pub struct RollbackIndex(usize);

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a, S> {
    _ctx: &'a Context,
    source: Required<S>,
}

impl<S> Builder<'_, S> {
    pub fn source(mut self, source: S) -> Self {
        self.source.set(source);
        self
    }
}

impl<'a, S> operation::Builder<'a, LoadConfiguration<S>> for Builder<'a, S>
where
    S: Source + Debug + Send + Sync,
{
    fn new(ctx: &'a Context) -> Self {
        Self {
            _ctx: ctx,
            source: Required::init(),
        }
    }

    fn finish(self) -> Result<LoadConfiguration<S>, crate::Error> {
        Ok(LoadConfiguration {
            source: self.source.require::<LoadConfiguration<S>>("source")?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Ok,
    // TODO: this actually contains <rpc-error> elements:
    //
    //  <rpc-reply
    //      xmlns=\"urn:ietf:params:xml:ns:netconf:base:1.0\"
    //      xmlns:junos=\"http://xml.juniper.net/junos/23.1R0/junos\"
    //      message-id=\"6\">
    //      <load-configuration-results>
    //          <rpc-error>
    //              <error-type>protocol</error-type>
    //              <error-tag>operation-failed</error-tag>
    //              <error-severity>error</error-severity>
    //              <error-message>
    //                  configuration database size limit exceeded
    //              </error-message>
    //          </rpc-error>
    //          <rpc-error>
    //              <error-type>protocol</error-type>
    //              <error-tag>operation-failed</error-tag>
    //              <error-severity>error</error-severity>
    //              <error-message>statement creation failed</error-message>
    //              <error-info>
    //                  <bad-element>route-filter</bad-element>
    //              </error-info>
    //          </rpc-error>
    //          <load-error-count>1</load-error-count>
    //      </load-configuration-results>
    //  </rpc-reply>
    //  ]]>]]>
    ErrorCount(usize),
}

impl ReadXml for Reply {
    #[tracing::instrument(skip_all, fields(tag = ?start.local_name()), level = "debug")]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let mut this = None;
        tracing::debug!("expecting <load-configuration-results>");
        loop {
            match reader.read_resolved_event()? {
                (ResolveResult::Bound(xmlns::BASE), Event::Start(tag))
                    if tag.local_name().as_ref() == b"load-configuration-results"
                        && this.is_none() =>
                {
                    let end = tag.to_end();
                    loop {
                        match reader.read_resolved_event()? {
                            (ResolveResult::Bound(xmlns::BASE), Event::Empty(tag))
                                if tag.local_name().as_ref() == b"ok" && this.is_none() =>
                            {
                                tracing::debug!(?tag);
                                this = Some(Self::Ok);
                            }
                            (ResolveResult::Bound(xmlns::BASE), Event::Start(tag))
                                if tag.local_name().as_ref() == b"load-error-count"
                                    && this.is_none() =>
                            {
                                tracing::debug!(?tag);
                                let count = reader
                                    .read_text(tag.to_end().name())?
                                    .parse::<usize>()
                                    .map_err(|err| ReadError::Other(err.into()))?;
                                this = Some(Self::ErrorCount(count));
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
        this.ok_or_else(|| ReadError::missing_element("rpc-reply", "load-configuration-results"))
    }
}

impl IntoResult for Reply {
    type Ok = ();
    fn into_result(self) -> Result<<Self as IntoResult>::Ok, crate::Error> {
        match self {
            Self::Ok => Ok(()),
            Self::ErrorCount(error_count) => Err(crate::Error::LoadConfiguration(error_count)),
        }
    }
}
