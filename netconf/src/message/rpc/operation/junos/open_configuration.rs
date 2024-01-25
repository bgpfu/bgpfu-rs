use std::{io::Write, sync::Arc};

use quick_xml::{events::BytesText, Writer};

use crate::{
    capabilities::{Capability, Requirements},
    message::{
        rpc::{
            operation::{self, params::Required},
            Empty, Operation,
        },
        WriteError, WriteXml,
    },
    session::Context,
};

/// Create a private copy of the candidate configuration or open the default instance or a
/// user-defined instance of the ephemeral configuration database.
///
/// See [Juniper documentation][junos-docs].
///
/// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-open-configuration.html
#[derive(Debug, Clone)]
pub struct OpenConfiguration {
    target: Target,
}

impl Operation for OpenConfiguration {
    const NAME: &'static str = "open-configuration";
    const REQUIRED_CAPABILITIES: Requirements =
        Requirements::One(Capability::JunosXmlManagementProtocol);
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

#[derive(Debug, Clone)]
pub enum Target {
    Private,
    Ephemeral(Ephemeral),
}

#[derive(Debug, Default, Clone)]
pub enum Ephemeral {
    #[default]
    Default,
    Named(Arc<str>),
}

impl WriteXml for OpenConfiguration {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        _ = writer
            .create_element(Self::NAME)
            .write_inner_content(|writer| self.target.write_xml(writer))?;
        Ok(())
    }
}

impl WriteXml for Target {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        match self {
            Self::Private => {
                _ = writer.create_element("private").write_empty()?;
                Ok(())
            }
            Self::Ephemeral(ephemeral) => ephemeral.write_xml(writer),
        }
    }
}

impl WriteXml for Ephemeral {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        _ = match self {
            Self::Default => writer.create_element("ephemeral").write_empty()?,
            Self::Named(name) => writer
                .create_element("ephemeral-instance")
                .write_text_content(BytesText::new(name))?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    _ctx: &'a Context,
    target: Required<Target>,
}

impl Builder<'_> {
    pub fn target(mut self, target: Target) -> Self {
        self.target.set(target);
        self
    }
}

impl<'a> operation::Builder<'a, OpenConfiguration> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            _ctx: ctx,
            target: Required::init(),
        }
    }

    fn finish(self) -> Result<OpenConfiguration, crate::Error> {
        Ok(OpenConfiguration {
            target: self.target.require::<OpenConfiguration>("target")?,
        })
    }
}
