use std::io::Write;

use quick_xml::Writer;

use crate::{
    capabilities::Requirements,
    message::{rpc::Empty, WriteError},
    session::Context,
    Error,
};

use super::{params::Required, Datastore, Operation, Url, WriteXml};

#[derive(Debug, Clone)]
pub struct DeleteConfig {
    target: Target,
}

impl Operation for DeleteConfig {
    const NAME: &'static str = "delete-config";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for DeleteConfig {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        Writer::new(writer)
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer.get_mut()))
                    .map(|_| ())
            })
            .map(|_| ())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    target: Required<Target>,
}

impl Builder<'_> {
    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        if matches!(target, Datastore::Running) {
            return Err(Error::DeleteRunningConfig);
        };
        target.try_as_target(self.ctx).map(|target| {
            self.target.set(Target::Datastore(target));
            self
        })
    }

    pub fn url<S: AsRef<str>>(mut self, url: S) -> Result<Self, Error> {
        Url::try_new(url, self.ctx).map(|url| {
            self.target.set(Target::Url(url));
            self
        })
    }
}

impl<'a> super::Builder<'a, DeleteConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            target: Required::init(),
        }
    }

    fn finish(self) -> Result<DeleteConfig, Error> {
        Ok(DeleteConfig {
            target: self.target.require::<DeleteConfig>("target")?,
        })
    }
}

#[derive(Debug, Clone)]
enum Target {
    Datastore(Datastore),
    Url(Url),
}

impl WriteXml for Target {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        match self {
            Self::Datastore(datastore) => datastore.write_xml(writer),
            Self::Url(url) => url.write_xml(writer),
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
