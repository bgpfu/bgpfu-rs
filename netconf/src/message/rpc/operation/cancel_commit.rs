use std::io::Write;

use quick_xml::{events::BytesText, Writer};

use crate::{
    capabilities::{Capability, Requirements},
    message::rpc::Empty,
    session::Context,
    Error,
};

use super::{Operation, Token, WriteXml};

#[derive(Debug, Clone)]
pub struct CancelCommit {
    persist_id: Option<Token>,
}

impl Operation for CancelCommit {
    const NAME: &'static str = "cancel-commit";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::One(Capability::ConfirmedCommitV1_1);

    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for CancelCommit {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let mut writer = Writer::new(writer);
        let elem = writer.create_element("cancel-commit");
        if let Some(ref token) = self.persist_id {
            _ = elem.write_inner_content(|writer| {
                _ = writer
                    .create_element("persist-id")
                    .write_text_content(BytesText::new(&token.to_string()))?;
                Ok::<_, Error>(())
            })?;
        } else {
            _ = elem.write_empty()?;
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    persist_id: Option<Token>,
}

impl Builder<'_> {
    pub fn persist_id(mut self, token: Option<Token>) -> Result<Self, Error> {
        let required_capabilities = Requirements::One(Capability::ConfirmedCommitV1_1);
        if required_capabilities.check(self.ctx.server_capabilities()) {
            self.persist_id = token;
            Ok(self)
        } else {
            Err(Error::UnsupportedOperationParameter(
                CancelCommit::NAME,
                "persist-id",
                required_capabilities,
            ))
        }
    }
}

impl<'a> super::Builder<'a, CancelCommit> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            persist_id: None,
        }
    }

    fn finish(self) -> Result<CancelCommit, Error> {
        Ok(CancelCommit {
            persist_id: self.persist_id,
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
    fn non_persisted_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: CancelCommit { persist_id: None },
        };
        let expect = r#"<rpc message-id="101"><cancel-commit/></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn persisted_request_to_xml() {
        let token = Token::generate();
        let req = Request {
            message_id: MessageId(101),
            operation: CancelCommit {
                persist_id: Some(token.clone()),
            },
        };
        let expect = format!(
            r#"<rpc message-id="101"><cancel-commit><persist-id>{token}</persist-id></cancel-commit></rpc>]]>]]>"#
        );
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
