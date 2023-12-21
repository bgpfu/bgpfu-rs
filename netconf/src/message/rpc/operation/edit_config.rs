use std::io::Write;

use quick_xml::Writer;

use crate::{capabilities::Capability, message::rpc::Empty, session::Context, Error};

use super::{Datastore, Operation, WriteXml};

#[derive(Debug, Clone)]
pub struct EditConfig {
    target: Datastore,
    source: Source,
    default_operation: DefaultOperation,
    error_option: ErrorOption,
    test_option: Option<TestOption>,
}

impl Operation for EditConfig {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for EditConfig {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("edit-config")
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer.get_mut()))?;
                if self.default_operation.is_non_default() {
                    _ = writer
                        .create_element("default-operation")
                        .write_inner_content(|writer| {
                            write!(writer.get_mut(), "{}", self.default_operation.as_str())?;
                            Ok::<_, Error>(())
                        })?;
                };
                if self.error_option.is_non_default() {
                    _ = writer
                        .create_element("error-option")
                        .write_inner_content(|writer| {
                            write!(writer.get_mut(), "{}", self.error_option.as_str())?;
                            Ok::<_, Error>(())
                        })?;
                };
                // TODO
                #[allow(clippy::redundant_pattern_matching)]
                if let Some(_) = self.test_option {
                    unreachable!();
                };
                self.source.write_xml(writer.get_mut())?;
                Ok::<_, Self::Error>(())
            })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a> {
    ctx: &'a Context,
    target: Option<Datastore>,
    source: Option<Source>,
    default_operation: DefaultOperation,
    error_option: ErrorOption,
    test_option: Option<TestOption>,
}

impl Builder<'_> {
    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        target.try_as_target(self.ctx).map(|target| {
            self.target = Some(target);
            self
        })
    }

    pub fn config(mut self, config: String) -> Self {
        self.source = Some(Source::Config(config));
        self
    }

    pub const fn default_operation(mut self, default_operation: DefaultOperation) -> Self {
        self.default_operation = default_operation;
        self
    }

    pub fn error_option(mut self, error_option: ErrorOption) -> Result<Self, Error> {
        error_option.try_use(self.ctx).map(|error_option| {
            self.error_option = error_option;
            self
        })
    }

    pub fn test_option(mut self, test_option: Option<TestOption>) -> Result<Self, Error> {
        todo!()
    }
}

impl<'a> super::Builder<'a, EditConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            target: None,
            source: None,
            default_operation: DefaultOperation::default(),
            error_option: ErrorOption::default(),
            test_option: None,
        }
    }

    fn finish(self) -> Result<EditConfig, Error> {
        let target = self
            .target
            .ok_or_else(|| Error::MissingOperationParameter("edit-config", "target"))?;
        let source = self
            .source
            .ok_or_else(|| Error::MissingOperationParameter("edit-config", "config"))?;
        Ok(EditConfig {
            target,
            source,
            default_operation: self.default_operation,
            error_option: self.error_option,
            test_option: self.test_option,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Source {
    Config(String),
}

impl WriteXml for Source {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let mut writer = Writer::new(writer);
        _ = match self {
            Self::Config(config) => {
                writer
                    .create_element("config")
                    .write_inner_content(|writer| {
                        writer
                            .get_mut()
                            .write_all(config.as_bytes())
                            .map_err(|err| Error::RpcRequestSerialization(err.into()))
                    })?
            }
        };
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DefaultOperation {
    #[default]
    Merge,
    Replace,
    None,
}

impl DefaultOperation {
    fn is_non_default(self) -> bool {
        self != Self::default()
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Replace => "replace",
            Self::None => "none",
        }
    }
}

// TODO: requires :validate:1.1 capability
#[derive(Debug, /* Default, */ Clone, Copy, PartialEq, Eq)]
pub enum TestOption {
    // #[default]
    // TestThenSet,
    // Set,
    // TestOnly,
}

// impl TestOption {
//     fn is_non_default(self) -> bool {
//         self != Self::default()
//     }
// }

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ErrorOption {
    #[default]
    StopOnError,
    ContinueOnError,
    RollbackOnError,
}

impl ErrorOption {
    fn is_non_default(self) -> bool {
        self != Self::default()
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::StopOnError => "stop-on-error",
            Self::ContinueOnError => "continue-on-error",
            Self::RollbackOnError => "rollback-on-error",
        }
    }

    fn try_use(self, ctx: &Context) -> Result<Self, Error> {
        let required_capability = match self {
            Self::StopOnError | Self::ContinueOnError => None,
            Self::RollbackOnError => Some(Capability::RollbackOnError),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedOperParameterValue(
                        "<edit-config>",
                        "<error-option>",
                        self.as_str(),
                        capability,
                    ))
                }
            },
        )
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
            operation: EditConfig {
                target: Datastore::Running,
                source: Source::Config("<foo>bar</foo>".to_string()),
                default_operation: DefaultOperation::default(),
                error_option: ErrorOption::default(),
                test_option: None,
            },
        };
        let expect = r#"<rpc message-id="101"><edit-config><target><running/></target><config><foo>bar</foo></config></edit-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
