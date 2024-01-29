use std::io::Write;

use quick_xml::Writer;

use crate::{capabilities::Requirements, message::WriteError, session::Context, Error};

use super::{params::Required, Datastore, EmptyReply, Operation, WriteXml};

#[derive(Debug, Clone, Copy)]
pub struct Lock {
    target: Datastore,
}

impl Operation for Lock {
    const NAME: &'static str = "lock";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type Reply = EmptyReply;
}

impl WriteXml for Lock {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer))
                    .map(|_| ())
            })
            .map(|_| ())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Unlock {
    target: Datastore,
}

impl Operation for Unlock {
    const NAME: &'static str = "unlock";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type Reply = EmptyReply;
}

impl WriteXml for Unlock {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer))
                    .map(|_| ())
            })
            .map(|_| ())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    target: Required<Datastore>,
}

impl<'a> Builder<'a> {
    const fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            target: Required::init(),
        }
    }

    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        target.try_as_lock_target(self.ctx).map(|target| {
            self.target.set(target);
            self
        })
    }
}

impl<'a> super::Builder<'a, Lock> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self::new(ctx)
    }

    fn finish(self) -> Result<Lock, Error> {
        Ok(Lock {
            target: self.target.require::<Lock>("target")?,
        })
    }
}

impl<'a> super::Builder<'a, Unlock> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self::new(ctx)
    }

    fn finish(self) -> Result<Unlock, Error> {
        Ok(Unlock {
            target: self.target.require::<Unlock>("target")?,
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
