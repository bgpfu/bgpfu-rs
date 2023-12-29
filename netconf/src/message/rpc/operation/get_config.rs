use std::io::Write;

use quick_xml::Writer;

use crate::{capabilities::Requirements, session::Context, Error};

use super::{params::Required, Datastore, Filter, Operation, Reply, WriteXml};

#[derive(Debug, Default, Clone)]
pub struct GetConfig {
    source: Datastore,
    filter: Option<Filter>,
}

impl Operation for GetConfig {
    const NAME: &'static str = "get-config";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type ReplyData = Reply;
}

impl WriteXml for GetConfig {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("source")
                    .write_inner_content(|writer| self.source.write_xml(writer.get_mut()))?;
                if let Some(ref filter) = self.filter {
                    filter.write_xml(writer.get_mut())?;
                };
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    source: Required<Datastore>,
    filter: Option<Filter>,
}

impl Builder<'_> {
    pub fn source(mut self, source: Datastore) -> Result<Self, Error> {
        source.try_as_source(self.ctx).map(|source| {
            self.source.set(source);
            self
        })
    }

    pub fn filter(mut self, filter: Option<Filter>) -> Result<Self, Error> {
        self.filter = filter.map(|filter| filter.try_use(self.ctx)).transpose()?;
        Ok(self)
    }
}

impl<'a> super::Builder<'a, GetConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            source: Required::init(),
            filter: None,
        }
    }

    fn finish(self) -> Result<GetConfig, Error> {
        Ok(GetConfig {
            source: self.source.require::<GetConfig>("source")?,
            filter: self.filter,
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
    fn default_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: GetConfig::default(),
        };
        let expect = r#"<rpc message-id="101"><get-config><source><running/></source></get-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
