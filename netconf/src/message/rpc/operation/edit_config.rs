use std::io::Write;

use quick_xml::Writer;

use crate::{message::rpc::Empty, session::Context, Error};

use super::{Datastore, Operation, WriteXml};

#[derive(Debug, Default, Clone)]
pub struct EditConfig {
    target: Datastore,
    config: String,
    default_operation: DefaultOperation,
    error_option: ErrorOption,
    test_option: Option<TestOption>,
}

impl EditConfig {
    #[must_use]
    pub fn new(
        target: Datastore,
        config: String,
        default_operation: Option<DefaultOperation>,
        error_option: Option<ErrorOption>,
        test_option: Option<TestOption>,
    ) -> Self {
        Self {
            target,
            config,
            default_operation: default_operation.unwrap_or_default(),
            error_option: error_option.unwrap_or_default(),
            test_option,
        }
    }
}

impl Operation for EditConfig {
    type Builder<'a> = Builder<'a>;
    type ReplyData = Empty;
}

impl WriteXml for EditConfig {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ =
            Writer::new(writer)
                .create_element("edit-config")
                .write_inner_content(|writer| {
                    _ = writer
                        .create_element("target")
                        .write_inner_content(|writer| self.target.write_xml(writer.get_mut()))?;
                    if self.default_operation.is_non_default() {
                        _ = writer
                            .create_element("default-operation")
                            .write_inner_content(|writer| {
                                self.default_operation.write_xml(writer.get_mut())
                            })?;
                    };
                    if self.error_option.is_non_default() {
                        _ = writer.create_element("error-option").write_inner_content(
                            |writer| self.error_option.write_xml(writer.get_mut()),
                        )?;
                    };
                    // TODO
                    #[allow(clippy::redundant_pattern_matching)]
                    if let Some(_) = self.test_option {
                        unreachable!();
                    };
                    _ = writer
                        .create_element("config")
                        .write_inner_content(|writer| {
                            writer
                                .get_mut()
                                .write_all(self.config.as_bytes())
                                .map_err(|err| Error::RpcRequestSerialization(err.into()))
                        })?;
                    Ok::<_, Self::Error>(())
                })?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Builder<'a> {
    ctx: &'a Context,
    target: Option<Datastore>,
    config: Option<String>,
    default_operation: DefaultOperation,
    error_option: ErrorOption,
    test_option: Option<TestOption>,
}

impl<'a> super::Builder<'a, EditConfig> for Builder<'a> {
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            target: None,
            config: None,
            default_operation: DefaultOperation::default(),
            error_option: ErrorOption::default(),
            test_option: None,
        }
    }

    fn finish(self) -> Result<EditConfig, Error> {
        let target = self
            .target
            .ok_or_else(|| Error::MissingOperationParameter("edit-config", "target"))?;
        let config = self
            .config
            .ok_or_else(|| Error::MissingOperationParameter("edit-config", "config"))?;
        Ok(EditConfig {
            target,
            config,
            default_operation: self.default_operation,
            error_option: self.error_option,
            test_option: self.test_option,
        })
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
}

impl WriteXml for DefaultOperation {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let name = match self {
            Self::Merge => "merge",
            Self::Replace => "replace",
            Self::None => "none",
        };
        _ = Writer::new(writer).create_element(name).write_empty()?;
        Ok(())
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
    // TODO: requires the :rollback-on-error capability
    // RollbackOnError,
}

impl ErrorOption {
    fn is_non_default(self) -> bool {
        self != Self::default()
    }
}

impl WriteXml for ErrorOption {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let name = match self {
            Self::StopOnError => "stop-on-error",
            Self::ContinueOnError => "continue-on-error",
            // TODO
            // Self::RollbackOnError => "rollback-on-error",
        };
        _ = Writer::new(writer).create_element(name).write_empty()?;
        Ok(())
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
            operation: EditConfig::new(
                Datastore::Running,
                "<foo>bar</foo>".to_string(),
                None,
                None,
                None,
            ),
        };
        let expect = r#"<rpc message-id="101"><edit-config><target><running/></target><config><foo>bar</foo></config></edit-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
