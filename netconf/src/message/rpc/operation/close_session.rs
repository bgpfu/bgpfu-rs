use std::io::Write;

use quick_xml::Writer;

use super::{EmptyReply, Operation, WriteXml};
use crate::{capabilities::Requirements, message::WriteError, session::Context, Error};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct CloseSession;

impl Operation for CloseSession {
    const NAME: &'static str = "close-session";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;
    type Builder<'a> = Builder;
    type Reply = EmptyReply;
}

impl WriteXml for CloseSession {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        _ = writer.create_element(Self::NAME).write_empty()?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Builder;

impl super::Builder<'_, CloseSession> for Builder {
    fn new(_: &Context) -> Self {
        Self
    }

    fn finish(self) -> Result<CloseSession, Error> {
        Ok(CloseSession)
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
            operation: CloseSession,
        };
        let expect = r#"<rpc message-id="101"><close-session/></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
