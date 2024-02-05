use std::{fmt::Debug, io::Write};

use quick_xml::{events::BytesText, Writer};

use crate::{
    capabilities::{Capability, Requirements},
    message::WriteError,
    session::Context,
    Error,
};

use super::{params::Required, Datastore, EmptyReply, Operation, Url, WriteXml};

#[derive(Debug, Clone)]
pub struct EditConfig<D> {
    target: Datastore,
    source: Source<D>,
    default_operation: DefaultOperation,
    error_option: ErrorOption,
    test_option: TestOption,
}

impl<D> Operation for EditConfig<D>
where
    D: WriteXml + Debug + Send + Sync,
{
    const NAME: &'static str = "edit-config";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;
    type Builder<'a> = Builder<'a, D>;
    type Reply = EmptyReply;
}

impl<D> WriteXml for EditConfig<D>
where
    D: WriteXml + Debug + Send + Sync,
{
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("target")
                    .write_inner_content(|writer| self.target.write_xml(writer))?;
                if self.default_operation.is_non_default() {
                    _ = writer
                        .create_element("default-operation")
                        .write_text_content(BytesText::new(self.default_operation.as_str()))?;
                };
                if self.error_option.is_non_default() {
                    _ = writer
                        .create_element("error-option")
                        .write_text_content(BytesText::new(self.error_option.as_str()))?;
                };
                if self.test_option.is_non_default() {
                    _ = writer
                        .create_element("test-option")
                        .write_text_content(BytesText::new(self.test_option.as_str()))?;
                };
                self.source.write_xml(writer)?;
                Ok(())
            })
            .map(|_| ())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder<'a, D> {
    ctx: &'a Context,
    target: Required<Datastore>,
    source: Required<Source<D>>,
    default_operation: DefaultOperation,
    error_option: ErrorOption,
    test_option: TestOption,
}

impl<D> Builder<'_, D>
where
    D: WriteXml + Debug + Send + Sync,
{
    pub fn target(mut self, target: Datastore) -> Result<Self, Error> {
        target.try_as_target(self.ctx).map(|target| {
            self.target.set(target);
            self
        })
    }

    pub fn config(mut self, config: D) -> Self {
        self.source.set(Source::Config(config));
        self
    }

    pub fn url<S: AsRef<str>>(mut self, url: S) -> Result<Self, Error> {
        Url::try_new(url, self.ctx).map(|url| {
            self.source.set(Source::Url(url));
            self
        })
    }

    pub const fn default_operation(mut self, default_operation: DefaultOperation) -> Self {
        self.default_operation = default_operation;
        self
    }

    pub fn error_option(mut self, error_option: ErrorOption) -> Result<Self, Error> {
        error_option.try_use::<D>(self.ctx).map(|error_option| {
            self.error_option = error_option;
            self
        })
    }

    pub fn test_option(mut self, test_option: TestOption) -> Result<Self, Error> {
        test_option.try_use::<D>(self.ctx).map(|test_option| {
            self.test_option = test_option;
            self
        })
    }
}

impl<'a, D> super::Builder<'a, EditConfig<D>> for Builder<'a, D>
where
    D: WriteXml + Debug + Send + Sync,
{
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            target: Required::init(),
            source: Required::init(),
            default_operation: DefaultOperation::default(),
            error_option: ErrorOption::default(),
            test_option: TestOption::default(),
        }
    }

    fn finish(self) -> Result<EditConfig<D>, Error> {
        Ok(EditConfig {
            target: self.target.require::<EditConfig<D>>("target")?,
            source: self.source.require::<EditConfig<D>>("config")?,
            default_operation: self.default_operation,
            error_option: self.error_option,
            test_option: self.test_option,
        })
    }
}

#[derive(Debug, Clone)]
pub enum Source<D> {
    Config(D),
    Url(Url),
}

impl<D> WriteXml for Source<D>
where
    D: WriteXml,
{
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        match self {
            Self::Config(config) => writer
                .create_element("config")
                .write_inner_content(|writer| config.write_xml(writer))
                .map(|_| ()),
            Self::Url(url) => url.write_xml(writer),
        }
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TestOption {
    #[default]
    TestThenSet,
    Set,
    TestOnly,
}

impl TestOption {
    fn is_non_default(self) -> bool {
        self != Self::default()
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::TestThenSet => "test-then-set",
            Self::Set => "set",
            Self::TestOnly => "test-only",
        }
    }

    fn try_use<D>(self, ctx: &Context) -> Result<Self, Error>
    where
        D: WriteXml + Debug + Send + Sync,
    {
        let required_capabilities = match self {
            Self::TestThenSet | Self::Set => {
                Requirements::Any(&[Capability::ValidateV1_0, Capability::ValidateV1_1])
            }
            Self::TestOnly => Requirements::One(Capability::ValidateV1_1),
        };
        if required_capabilities.check(ctx.server_capabilities()) {
            Ok(self)
        } else {
            Err(Error::UnsupportedOperParameterValue(
                EditConfig::<D>::NAME,
                "<test-option>",
                self.as_str(),
                required_capabilities,
            ))
        }
    }
}

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

    fn try_use<D>(self, ctx: &Context) -> Result<Self, Error>
    where
        D: WriteXml + Debug + Send + Sync,
    {
        let required_capabilities = match self {
            Self::StopOnError | Self::ContinueOnError => Requirements::None,
            Self::RollbackOnError => Requirements::One(Capability::RollbackOnError),
        };
        if required_capabilities.check(ctx.server_capabilities()) {
            Ok(self)
        } else {
            Err(Error::UnsupportedOperParameterValue(
                EditConfig::<D>::NAME,
                "<error-option>",
                self.as_str(),
                required_capabilities,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{
        rpc::{operation::Opaque, MessageId, Request},
        ClientMsg,
    };

    #[test]
    fn request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: EditConfig {
                target: Datastore::Running,
                source: Source::<Opaque>::Config("<foo>bar</foo>".into()),
                default_operation: DefaultOperation::default(),
                error_option: ErrorOption::default(),
                test_option: TestOption::default(),
            },
        };
        let expect = r#"<rpc message-id="101"><edit-config><target><running/></target><config><foo>bar</foo></config></edit-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
