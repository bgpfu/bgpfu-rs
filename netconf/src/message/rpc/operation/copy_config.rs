use std::io::Write;

use quick_xml::Writer;

use crate::{capabilities::Requirements, message::rpc::Empty, session::Context, Error};

use super::{params::Required, Datastore, Operation, Source, WriteXml};

#[derive(Debug, Clone)]
pub struct CopyConfig {
    target: Target,
    source: Source,
}

impl Operation for CopyConfig {
    const NAME: &'static str = "copy-config";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for CopyConfig {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer.get_mut()))?;
                _ = writer
                    .create_element("source")
                    .write_inner_content(|writer| self.source.write_xml(writer.get_mut()))?;
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    target: Required<Target>,
    source: Required<Source>,
}

impl Builder<'_> {
    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        target.try_as_target(self.ctx).map(|target| {
            self.target.set(Target::Datastore(target));
            self
        })
    }

    pub fn source(mut self, source: Datastore) -> Result<Self, Error> {
        source.try_as_source(self.ctx).map(|source| {
            self.source.set(Source::Datastore(source));
            self
        })
    }

    pub fn config(mut self, config: String) -> Self {
        self.source.set(Source::Config(config));
        self
    }
}

impl<'a> super::Builder<'a, CopyConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            target: Required::init(),
            source: Required::init(),
        }
    }

    fn finish(self) -> Result<CopyConfig, Error> {
        Ok(CopyConfig {
            target: self.target.require::<CopyConfig>("target")?,
            source: self.source.require::<CopyConfig>("source")?,
        })
    }
}

#[derive(Debug, Clone)]
enum Target {
    Datastore(Datastore),
}

impl WriteXml for Target {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        match self {
            Self::Datastore(datastore) => datastore.write_xml(writer),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{
        rpc::{MessageId, Request},
        ClientMsg,
    };

    #[test]
    fn request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: CopyConfig {
                target: Target::Datastore(Datastore::Running),
                source: Source::Config("<foo>bar</foo>".to_string()),
            },
        };
        let expect = r#"<rpc message-id="101"><copy-config><target><running/></target><source><config><foo>bar</foo></config></source></copy-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
