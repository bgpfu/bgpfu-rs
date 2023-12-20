use std::io::Write;

use quick_xml::Writer;

use crate::{message::rpc::Empty, session::Context, Error};

use super::{Datastore, Operation, WriteXml};

#[derive(Debug, Clone)]
pub struct DeleteConfig {
    target: Target,
}

impl Operation for DeleteConfig {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for DeleteConfig {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("delete-config")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer.get_mut()))?;
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    target: Option<Target>,
}

impl Builder<'_> {
    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        if matches!(target, Datastore::Running) {
            return Err(Error::DeleteRunningConfig);
        };
        target.try_as_target(self.ctx).map(|target| {
            self.target = Some(Target::Datastore(target));
            self
        })
    }
}

impl<'a> super::Builder<'a, DeleteConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self { ctx, target: None }
    }

    fn finish(self) -> Result<DeleteConfig, Error> {
        let target = self
            .target
            .ok_or_else(|| Error::MissingOperationParameter("delete-config", "target"))?;
        Ok(DeleteConfig { target })
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
            operation: DeleteConfig {
                target: Target::Datastore(Datastore::Running),
            },
        };
        let expect = r#"<rpc message-id="101"><delete-config><target><running/></target></delete-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
