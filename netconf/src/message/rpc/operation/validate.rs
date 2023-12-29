use std::io::Write;

use quick_xml::Writer;

use crate::{
    capabilities::{Capability, Requirements},
    message::rpc::Empty,
    session::Context,
    Error,
};

use super::{params::Required, Datastore, Operation, Source, WriteXml};

#[derive(Debug, Clone)]
pub struct Validate {
    source: Source,
}

impl Operation for Validate {
    const NAME: &'static str = "validate";
    const REQUIRED_CAPABILITIES: Requirements =
        Requirements::Any(&[Capability::ValidateV1_0, Capability::ValidateV1_1]);

    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for Validate {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
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
    source: Required<Source>,
}

impl Builder<'_> {
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

impl<'a> super::Builder<'a, Validate> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            source: Required::init(),
        }
    }

    fn finish(self) -> Result<Validate, Error> {
        Ok(Validate {
            source: self.source.require::<Validate>("source")?,
        })
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
            operation: Validate {
                source: Source::Datastore(Datastore::Candidate),
            },
        };
        let expect = r#"<rpc message-id="101"><validate><source><candidate/></source></validate></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
