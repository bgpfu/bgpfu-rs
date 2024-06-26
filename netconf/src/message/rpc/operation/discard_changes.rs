use std::io::Write;

use quick_xml::Writer;

use crate::{
    capabilities::{Capability, Requirements},
    message::WriteError,
    session::Context,
    Error,
};

use super::{EmptyReply, Operation, WriteXml};

#[derive(Debug, Clone, Copy)]
pub struct DiscardChanges {
    // zero-sized private field to prevent direct construction
    _inner: (),
}

impl Operation for DiscardChanges {
    const NAME: &'static str = "discard-changes";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::One(Capability::Candidate);

    type Builder<'a> = Builder<'a>;
    type Reply = EmptyReply;
}

impl WriteXml for DiscardChanges {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        _ = writer.create_element("discard-changes").write_empty()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    _ctx: &'a Context,
}

impl<'a> super::Builder<'a, DiscardChanges> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self { _ctx: ctx }
    }

    fn finish(self) -> Result<DiscardChanges, Error> {
        Ok(DiscardChanges { _inner: () })
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
            operation: DiscardChanges { _inner: () },
        };
        let expect = r#"<rpc message-id="101"><discard-changes/></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
