use std::io::Write;

use quick_xml::Writer;

use crate::{
    message::rpc::Empty,
    session::{Context, SessionId},
    Error,
};

use super::{Datastore, Operation, WriteXml};

#[derive(Debug, Clone, Copy)]
pub struct KillSession {
    session_id: SessionId,
}

impl Operation for KillSession {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for KillSession {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("kill-session")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("session-id")
                    .write_inner_content(|writer| {
                        write!(writer.get_mut(), "{}", self.session_id)?;
                        Ok::<_, Self::Error>(())
                    })?;
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    session_id: Option<SessionId>,
}

impl Builder<'_> {
    pub fn session_id(mut self, session_id: u32) -> Result<Self, Error> {
        SessionId::new(session_id)
            .ok_or_else(|| Error::InvalidSessionId(session_id))
            .and_then(|session_id| {
                if session_id == self.ctx.session_id() {
                    Err(Error::KillCurrentSession)
                } else {
                    self.session_id = Some(session_id);
                    Ok(self)
                }
            })
    }
}

impl<'a> super::Builder<'a, KillSession> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            session_id: None,
        }
    }

    fn finish(self) -> Result<KillSession, Error> {
        let session_id = self
            .session_id
            .ok_or_else(|| Error::MissingOperationParameter("kill-session", "session-id"))?;
        Ok(KillSession { session_id })
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
