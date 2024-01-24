use std::{io::Write, time::Duration};

use quick_xml::{events::BytesText, Writer};

use crate::{
    capabilities::{Capability, Requirements},
    message::{rpc::Empty, WriteError},
    session::Context,
    Error,
};

use super::{Operation, Token, WriteXml};

#[derive(Debug, Clone)]
pub struct Commit {
    confirmed: bool,
    confirm_timeout: Timeout,
    persist: Option<Token>,
    persist_id: Option<Token>,
}

impl Operation for Commit {
    const NAME: &'static str = "commit";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::One(Capability::Candidate);

    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for Commit {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let elem = writer.create_element("commit");
        if self.confirmed {
            elem.write_inner_content(|writer| {
                _ = writer.create_element("confirmed").write_empty()?;
                if self.confirm_timeout != Timeout::default() {
                    _ = writer
                        .create_element("confirm-timeout")
                        .write_text_content(BytesText::new(
                            &self.confirm_timeout.0.as_secs().to_string(),
                        ))?;
                };
                if let Some(ref token) = self.persist {
                    _ = writer
                        .create_element("persist")
                        .write_text_content(BytesText::new(&token.to_string()))?;
                }
                Ok(())
            })
            .map(|_| ())
        } else if let Some(ref token) = self.persist_id {
            elem.write_inner_content(|writer| {
                _ = writer
                    .create_element("persist-id")
                    .write_text_content(BytesText::new(&token.to_string()))?;
                Ok(())
            })
            .map(|_| ())
        } else {
            _ = elem.write_empty()?;
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Timeout(Duration);

impl Default for Timeout {
    fn default() -> Self {
        Self(Duration::from_secs(600))
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    confirmed: bool,
    confirm_timeout: Timeout,
    persist: Option<Token>,
    persist_id: Option<Token>,
}

impl Builder<'_> {
    pub fn confirmed(mut self, confirmed: bool) -> Result<Self, Error> {
        self.try_use_confirmed("confirmed").map(|()| {
            self.confirmed = confirmed;
            self
        })
    }

    pub fn confirm_timeout(mut self, timeout: Duration) -> Result<Self, Error> {
        self.try_use_confirmed("confirm-timeout").map(|()| {
            self.confirm_timeout = Timeout(timeout);
            self
        })
    }

    pub fn persist(mut self, token: Option<Token>) -> Result<Self, Error> {
        self.try_use_persist("persist").map(|()| {
            self.persist = token;
            self
        })
    }

    pub fn persist_id(mut self, token: Option<Token>) -> Result<Self, Error> {
        self.try_use_persist("persist-id").map(|()| {
            self.persist_id = token;
            self
        })
    }

    fn try_use_confirmed(&self, param_name: &'static str) -> Result<(), Error> {
        let required_capabilities = Requirements::Any(&[
            Capability::ConfirmedCommitV1_0,
            Capability::ConfirmedCommitV1_1,
        ]);
        self.try_use(required_capabilities, param_name)
    }

    fn try_use_persist(&self, param_name: &'static str) -> Result<(), Error> {
        let required_capabilities = Requirements::One(Capability::ConfirmedCommitV1_1);
        self.try_use(required_capabilities, param_name)
    }

    fn try_use(
        &self,
        required_capabilities: Requirements,
        param_name: &'static str,
    ) -> Result<(), Error> {
        required_capabilities
            .check(self.ctx.server_capabilities())
            .then_some(())
            .ok_or_else(|| {
                Error::UnsupportedOperationParameter(
                    Commit::NAME,
                    param_name,
                    required_capabilities,
                )
            })
    }
}

impl<'a> super::Builder<'a, Commit> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            confirmed: false,
            confirm_timeout: Timeout::default(),
            persist: None,
            persist_id: None,
        }
    }

    fn finish(self) -> Result<Commit, Error> {
        if self.confirmed && self.persist_id.is_some() {
            return Err(Error::IncompatibleOperationParameters(
                "commit",
                vec!["confirmed = true", "persist-id"],
            ));
        }
        if !self.confirmed && self.persist.is_some() {
            return Err(Error::IncompatibleOperationParameters(
                "commit",
                vec!["confirmed = false", "persist"],
            ));
        }
        Ok(Commit {
            confirmed: self.confirmed,
            confirm_timeout: self.confirm_timeout,
            persist: self.persist,
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
    fn unconfirmed_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Commit {
                confirmed: false,
                confirm_timeout: Timeout::default(),
                persist: None,
                persist_id: None,
            },
        };
        let expect = r#"<rpc message-id="101"><commit/></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn confirmed_request_to_xml() {
        let token = Token::generate();
        let req = Request {
            message_id: MessageId(101),
            operation: Commit {
                confirmed: true,
                confirm_timeout: Timeout::default(),
                persist: Some(token.clone()),
                persist_id: None,
            },
        };
        let expect = format!(
            r#"<rpc message-id="101"><commit><confirmed/><persist>{token}</persist></commit></rpc>]]>]]>"#
        );
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn confirmed_with_timeout_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Commit {
                confirmed: true,
                confirm_timeout: Timeout(Duration::from_secs(60)),
                persist: None,
                persist_id: None,
            },
        };
        let expect = r#"<rpc message-id="101"><commit><confirmed/><confirm-timeout>60</confirm-timeout></commit></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
