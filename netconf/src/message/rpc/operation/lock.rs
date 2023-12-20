use std::io::Write;

use quick_xml::Writer;

use crate::{message::rpc::Empty, session::Context, Error};

use super::{Datastore, Operation, WriteXml};

#[derive(Debug, Clone, Copy)]
pub struct Lock {
    target: Datastore,
}

impl Operation for Lock {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for Lock {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("lock")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer.get_mut()))?;
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Unlock {
    target: Datastore,
}

impl Operation for Unlock {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for Unlock {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("unlock")
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
    target: Option<Datastore>,
}

impl Builder<'_> {
    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        target.try_as_lock_target(self.ctx).map(|target| {
            self.target = Some(target);
            self
        })
    }
}

impl<'a> super::Builder<'a, Lock> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self { ctx, target: None }
    }

    fn finish(self) -> Result<Lock, Error> {
        let target = self
            .target
            .ok_or_else(|| Error::MissingOperationParameter("lock", "target"))?;
        Ok(Lock { target })
    }
}

impl<'a> super::Builder<'a, Unlock> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self { ctx, target: None }
    }

    fn finish(self) -> Result<Unlock, Error> {
        let target = self
            .target
            .ok_or_else(|| Error::MissingOperationParameter("unlock", "target"))?;
        Ok(Unlock { target })
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
    fn lock_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Lock {
                target: Datastore::Running,
            },
        };
        let expect =
            r#"<rpc message-id="101"><lock><target><running/></target></lock></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn unlock_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Unlock {
                target: Datastore::Running,
            },
        };
        let expect =
            r#"<rpc message-id="101"><unlock><target><running/></target></unlock></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
