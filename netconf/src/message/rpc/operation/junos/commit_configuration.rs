use std::{fmt::Debug, io::Write, sync::Arc, time::Duration};

use chrono::{NaiveDateTime, NaiveTime};
use quick_xml::{events::BytesText, Writer};

use crate::{
    capabilities::{Capability, Requirements},
    message::{
        rpc::{
            operation::{self, Timeout},
            EmptyReply, Operation,
        },
        WriteError, WriteXml,
    },
    session::Context,
};

/// Request that the NETCONF or Junos XML protocol server perform one of the variants of the commit
/// operation on the candidate configuration, a private copy of the candidate configuration, or an
/// open instance of the ephemeral configuration database.
///
/// See [Juniper documentation][junos-docs].
///
/// [junos-docs]: https://www.juniper.net/documentation/us/en/software/junos/netconf/junos-xml-protocol/topics/ref/tag/junos-xml-protocol-commit-configuration.html
#[derive(Debug, Clone)]
pub struct CommitConfiguration {
    check: bool,
    at_time: Option<AtTime>,
    confirm: Option<Confirm>,
    log: Option<Message>,
    synchronize: Option<Synchronize>,
}

impl Operation for CommitConfiguration {
    const NAME: &'static str = "commit-configuration";
    const REQUIRED_CAPABILITIES: Requirements =
        Requirements::One(Capability::JunosXmlManagementProtocol);
    type Builder<'a> = Builder<'a>;
    // TODO: WTF! That's not what the docs say :-)
    type Reply = EmptyReply;
}

impl CommitConfiguration {
    fn has_options(&self) -> bool {
        [
            self.check,
            self.at_time.is_some(),
            self.confirm.is_some(),
            self.log.is_some(),
            self.synchronize.is_some(),
        ]
        .iter()
        .any(|opt| *opt)
    }
}

impl WriteXml for CommitConfiguration {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let elem = writer.create_element(Self::NAME);
        _ = if self.has_options() {
            elem.write_inner_content(|writer| {
                if self.check {
                    _ = writer.create_element("check").write_empty()?;
                }
                if let Some(ref at_time) = self.at_time {
                    at_time.write_xml(writer)?;
                }
                if let Some(ref confirm) = self.confirm {
                    confirm.write_xml(writer)?;
                }
                if let Some(ref message) = self.log {
                    message.write_xml(writer)?;
                }
                if let Some(ref synchronize) = self.synchronize {
                    synchronize.write_xml(writer)?;
                }
                Ok::<_, WriteError>(())
            })?
        } else {
            elem.write_empty()?
        };
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AtTime {
    AtReboot,
    TodayAt(NaiveTime),
    At(NaiveDateTime),
}

impl WriteXml for AtTime {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let value = match self {
            Self::AtReboot => BytesText::new("reboot"),
            Self::TodayAt(time) => {
                BytesText::new(&time.format("%H:%M:%S").to_string()).into_owned()
            }
            Self::At(date_time) => {
                BytesText::new(&date_time.format("%Y-%m-%d %H:%M:%S").to_string()).into_owned()
            }
        };
        _ = writer.create_element("at-time").write_text_content(value)?;
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Confirm {
    timeout: Timeout,
}

impl WriteXml for Confirm {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        _ = writer.create_element("confirmed").write_empty()?;
        if self.timeout != Timeout::default() {
            _ = writer
                .create_element("confirm-timeout")
                .write_text_content(self.timeout.minutes())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Message {
    inner: Arc<str>,
}

impl WriteXml for Message {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        _ = writer
            .create_element("log")
            .write_text_content(BytesText::new(&self.inner))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Synchronize {
    force: bool,
}

// TODO:
// The juniper docs are un-clear as to whether the <synchronize> and <force-synchronize> tags
// should be used together or alternately... to check!
impl WriteXml for Synchronize {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        let tag = if self.force {
            "force-synchronize"
        } else {
            "synchronize"
        };
        _ = writer.create_element(tag).write_empty()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    _ctx: &'a Context,
    check: bool,
    at_time: Option<AtTime>,
    confirm: Option<Confirm>,
    log: Option<Message>,
    synchronize: Option<Synchronize>,
}

impl Builder<'_> {
    pub const fn check(mut self, check: bool) -> Self {
        self.check = check;
        self
    }

    pub const fn now(mut self) -> Self {
        self.at_time = None;
        self
    }

    pub const fn at_reboot(mut self) -> Self {
        self.at_time = Some(AtTime::AtReboot);
        self
    }

    pub const fn today_at(mut self, time: NaiveTime) -> Self {
        self.at_time = Some(AtTime::TodayAt(time));
        self
    }

    pub const fn at(mut self, date_time: NaiveDateTime) -> Self {
        self.at_time = Some(AtTime::At(date_time));
        self
    }

    pub fn confirmed(mut self, confirmed: bool) -> Self {
        self.confirm = confirmed.then_some(Confirm::default());
        self
    }

    pub const fn confirmed_with_timeout(mut self, timeout: Duration) -> Self {
        self.confirm = Some(Confirm {
            timeout: Timeout(timeout),
        });
        self
    }

    pub fn with_log_message<M: AsRef<str>>(mut self, message: M) -> Self {
        self.log = Some(Message {
            inner: message.as_ref().into(),
        });
        self
    }

    pub const fn synchronize(mut self, force: bool) -> Self {
        self.synchronize = Some(Synchronize { force });
        self
    }
}

impl<'a> operation::Builder<'a, CommitConfiguration> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            _ctx: ctx,
            check: false,
            at_time: None,
            confirm: None,
            log: None,
            synchronize: None,
        }
    }

    fn finish(self) -> Result<CommitConfiguration, crate::Error> {
        Ok(CommitConfiguration {
            check: self.check,
            at_time: self.at_time,
            confirm: self.confirm,
            log: self.log,
            synchronize: self.synchronize,
        })
    }
}
