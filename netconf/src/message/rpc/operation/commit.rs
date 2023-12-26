use std::{io::Write, time::Duration};

use quick_xml::{events::BytesText, Writer};

use crate::{capabilities::Capability, message::rpc::Empty, session::Context, Error};

use super::{Operation, Token, WriteXml};

#[derive(Debug, Clone)]
pub struct Commit {
    confirmed: bool,
    confirm_timeout: Timeout,
    persist: Option<Token>,
    persist_id: Option<Token>,
}

impl Operation for Commit {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for Commit {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let mut writer = Writer::new(writer);
        let elem = writer.create_element("commit");
        if self.confirmed {
            _ = elem.write_inner_content(|writer| {
                _ = writer.create_element("confirmed").write_empty()?;
                if self.confirm_timeout != Timeout::default() {
                    _ = writer
                        .create_element("confirm-timeout")
                        .write_inner_content(|writer| {
                            write!(writer.get_mut(), "{}", self.confirm_timeout.0.as_secs())?;
                            Ok::<_, Error>(())
                        })?;
                };
                if let Some(ref token) = self.persist {
                    _ = writer
                        .create_element("persist")
                        .write_text_content(BytesText::new(&token.to_string()))?;
                }
                Ok::<_, Error>(())
            })?;
        } else if let Some(ref token) = self.persist_id {
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
        if confirmed && !self.can_use_confirmed() {
            Err(Error::UnsupportedOperationParameter(
                "<commit>",
                "<confirmed/>",
                Capability::ConfirmedCommitV1_0,
            ))
        } else {
            self.confirmed = confirmed;
            Ok(self)
        }
    }

    pub fn confirm_timeout(mut self, timeout: Duration) -> Result<Self, Error> {
        if self.can_use_confirmed() {
            self.confirm_timeout = Timeout(timeout);
            Ok(self)
        } else {
            Err(Error::UnsupportedOperationParameter(
                "<commit>",
                "<confirm-timeout>",
                Capability::ConfirmedCommitV1_0,
            ))
        }
    }

    fn can_use_confirmed(&self) -> bool {
        self.ctx.server_capabilities().contains_any(&[
            &Capability::ConfirmedCommitV1_0,
            &Capability::ConfirmedCommitV1_1,
        ])
    }

    fn can_use_persist(&self) -> bool {
        self.ctx
            .server_capabilities()
            .contains(&Capability::ConfirmedCommitV1_1)
    }

    pub fn persist(mut self, token: Option<Token>) -> Result<Self, Error> {
        if token.is_some() && !self.can_use_persist() {
            Err(Error::UnsupportedOperationParameter(
                "<commit>",
                "<persist>",
                Capability::ConfirmedCommitV1_1,
            ))
        } else {
            self.persist = token;
            Ok(self)
        }
    }

    pub fn persist_id(mut self, token: Option<Token>) -> Result<Self, Error> {
        if token.is_some() && !self.can_use_persist() {
            Err(Error::UnsupportedOperationParameter(
                "<commit>",
                "<persist-id>",
                Capability::ConfirmedCommitV1_1,
            ))
        } else {
            self.persist_id = token;
            Ok(self)
        }
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
        self.ctx
            .try_operation(&[&Capability::Candidate], "<commit/>", || {
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
