use std::io::Write;

use quick_xml::{events::BytesText, Writer};

use crate::{
    capabilities::Requirements,
    message::{rpc::Empty, WriteError},
    session::{Context, SessionId},
    Error,
};

use super::{params::Required, Operation, WriteXml};

#[derive(Debug, Clone, Copy)]
pub struct KillSession {
    session_id: SessionId,
}

impl Operation for KillSession {
    const NAME: &'static str = "kill-session";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for KillSession {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        Writer::new(writer)
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("session-id")
                    .write_text_content(BytesText::new(&self.session_id.to_string()))?;
                Ok(())
            })
            .map(|_| ())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    session_id: Required<SessionId>,
}

impl Builder<'_> {
    pub fn session_id(mut self, session_id: u32) -> Result<Self, Error> {
        SessionId::new(session_id)
            .ok_or_else(|| Error::InvalidSessionId(session_id))
            .and_then(|session_id| {
                if session_id == self.ctx.session_id() {
                    Err(Error::KillCurrentSession)
                } else {
                    self.session_id.set(session_id);
                    Ok(self)
                }
            })
    }
}

impl<'a> super::Builder<'a, KillSession> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            session_id: Required::init(),
        }
    }

    fn finish(self) -> Result<KillSession, Error> {
        Ok(KillSession {
            session_id: self.session_id.require::<KillSession>("session-id")?,
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
            operation: KillSession {
                session_id: SessionId::new(999).unwrap(),
            },
        };
        let expect = r#"<rpc message-id="101"><kill-session><session-id>999</session-id></kill-session></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
