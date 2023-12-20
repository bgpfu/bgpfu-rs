use std::{io::Write, time::Duration};

use quick_xml::Writer;

use crate::{capabilities::Capability, message::rpc::Empty, session::Context, Error};

use super::{Operation, WriteXml};

#[derive(Debug, Clone, Copy)]
pub struct Commit {
    confirmed: bool,
    confirm_timeout: Timeout,
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
}

impl Builder<'_> {
    pub fn confirmed(mut self, confirmed: bool) -> Result<Self, Error> {
        if confirmed && !self.can_use_confirmed() {
            Err(Error::UnsupportedOperationParameter(
                "<commit>",
                "<confirmed/>",
                Capability::ConfirmedCommit,
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
                Capability::ConfirmedCommit,
            ))
        }
    }

    fn can_use_confirmed(&self) -> bool {
        self.ctx
            .server_capabilities()
            .contains(&Capability::ConfirmedCommit)
    }
}

impl<'a> super::Builder<'a, Commit> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            confirmed: false,
            confirm_timeout: Timeout::default(),
        }
    }

    fn finish(self) -> Result<Commit, Error> {
        self.ctx
            .try_operation(Capability::Candidate, "<commit/>", || {
                Ok(Commit {
                    confirmed: self.confirmed,
                    confirm_timeout: self.confirm_timeout,
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
            },
        };
        let expect = r#"<rpc message-id="101"><commit/></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn confirmed_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Commit {
                confirmed: true,
                confirm_timeout: Timeout::default(),
            },
        };
        let expect = r#"<rpc message-id="101"><commit><confirmed/></commit></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }

    #[test]
    fn confirmed_with_timeout_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Commit {
                confirmed: true,
                confirm_timeout: Timeout(Duration::from_secs(60)),
            },
        };
        let expect = r#"<rpc message-id="101"><commit><confirmed/><confirm-timeout>60</confirm-timeout></commit></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
