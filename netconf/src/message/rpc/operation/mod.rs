use std::{
    fmt::{self, Debug, Display},
    io::Write,
    sync::Arc,
};

use iri_string::types::UriStr;
use quick_xml::{
    events::{BytesStart, BytesText},
    NsReader, Writer,
};
use uuid::Uuid;

use crate::{
    capabilities::{Capability, Requirements},
    message::{ReadError, ReadXml, WriteError, WriteXml},
    session::Context,
    Error,
};

pub trait Operation: Debug + WriteXml + Send + Sync + Sized {
    const NAME: &'static str;
    const REQUIRED_CAPABILITIES: Requirements;

    type Builder<'a>: Builder<'a, Self>;
    type ReplyData: ReplyData;

    fn new<'a, F>(ctx: &'a Context, build_fn: F) -> Result<Self, Error>
    where
        F: Fn(Self::Builder<'a>) -> Result<Self, Error>,
    {
        Self::REQUIRED_CAPABILITIES
            .check(ctx.server_capabilities())
            .then(|| Self::Builder::new(ctx).build(build_fn))
            .ok_or_else(|| Error::UnsupportedOperation(Self::NAME, Self::REQUIRED_CAPABILITIES))?
    }
}

pub trait Builder<'a, O: Operation>: Debug + Sized {
    fn new(ctx: &'a Context) -> Self;

    fn finish(self) -> Result<O, Error>;

    fn build<F>(self, build_fn: F) -> Result<O, Error>
    where
        F: Fn(Self) -> Result<O, Error>,
    {
        build_fn(self)
    }
}

pub trait ReplyData: Debug + ReadXml + Sized {
    type Ok;

    fn from_ok() -> Result<Self::Ok, Error>;
    fn into_result(self) -> Result<Self::Ok, Error>;
}

mod params;

pub mod get;
#[doc(inline)]
pub use self::get::Get;

pub mod get_config;
#[doc(inline)]
pub use self::get_config::GetConfig;

pub mod edit_config;
#[doc(inline)]
pub use self::edit_config::EditConfig;

pub mod copy_config;
#[doc(inline)]
pub use self::copy_config::CopyConfig;

pub mod delete_config;
#[doc(inline)]
pub use self::delete_config::DeleteConfig;

pub mod lock;
#[doc(inline)]
pub use self::lock::{Lock, Unlock};

pub mod kill_session;
#[doc(inline)]
pub use self::kill_session::KillSession;

pub mod commit;
#[doc(inline)]
pub use self::commit::Commit;

pub mod cancel_commit;
#[doc(inline)]
pub use self::cancel_commit::CancelCommit;

pub mod discard_changes;
#[doc(inline)]
pub use self::discard_changes::DiscardChanges;

pub mod validate;
#[doc(inline)]
pub use self::validate::Validate;

pub(crate) mod close_session;
pub(crate) use self::close_session::CloseSession;

#[derive(Debug, Default, Copy, Clone)]
pub enum Datastore {
    #[default]
    Running,
    Candidate,
    Startup,
}

impl Datastore {
    fn try_as_source(self, ctx: &Context) -> Result<Self, Error> {
        let required_capabilities = match self {
            Self::Running => Requirements::None,
            Self::Candidate => Requirements::One(Capability::Candidate),
            Self::Startup => Requirements::One(Capability::Startup),
        };
        if required_capabilities.check(ctx.server_capabilities()) {
            Ok(self)
        } else {
            Err(Error::UnsupportedSource(self, required_capabilities))
        }
    }

    fn try_as_target(self, ctx: &Context) -> Result<Self, Error> {
        let required_capabilities = match self {
            Self::Running => Requirements::One(Capability::WritableRunning),
            Self::Candidate => Requirements::One(Capability::Candidate),
            Self::Startup => Requirements::One(Capability::Startup),
        };
        if required_capabilities.check(ctx.server_capabilities()) {
            Ok(self)
        } else {
            Err(Error::UnsupportedTarget(self, required_capabilities))
        }
    }

    fn try_as_lock_target(self, ctx: &Context) -> Result<Self, Error> {
        let required_capabilities = match self {
            Self::Running => Requirements::None,
            Self::Candidate => Requirements::One(Capability::Candidate),
            Self::Startup => Requirements::One(Capability::Startup),
        };
        if required_capabilities.check(ctx.server_capabilities()) {
            Ok(self)
        } else {
            Err(Error::UnsupportedLockTarget(self, required_capabilities))
        }
    }
}

impl WriteXml for Datastore {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        let mut writer = Writer::new(writer);
        _ = match self {
            Self::Running => writer.create_element("running").write_empty()?,
            Self::Candidate => writer.create_element("candidate").write_empty()?,
            Self::Startup => writer.create_element("startup").write_empty()?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Source {
    Datastore(Datastore),
    Config(String),
    Url(Url),
}

impl WriteXml for Source {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        match self {
            Self::Datastore(datastore) => datastore.write_xml(writer)?,
            Self::Config(config) => {
                _ = Writer::new(writer)
                    .create_element("config")
                    .write_inner_content(|writer| {
                        writer
                            .get_mut()
                            .write_all(config.as_bytes())
                            .map_err(|err| WriteError::Other(err.into()))
                    })?;
            }
            Self::Url(url) => url.write_xml(writer)?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum Filter {
    Subtree(String),
    XPath(String),
}

impl Filter {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Subtree(_) => "subtree",
            Self::XPath(_) => "xpath",
        }
    }

    fn try_use(self, ctx: &Context) -> Result<Self, Error> {
        let required_capabilities = match self {
            Self::Subtree(_) => Requirements::None,
            Self::XPath(_) => Requirements::One(Capability::XPath),
        };
        if required_capabilities.check(ctx.server_capabilities()) {
            Ok(self)
        } else {
            Err(Error::UnsupportedFilterType(
                self.as_str(),
                required_capabilities,
            ))
        }
    }
}

impl WriteXml for Filter {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        let mut writer = Writer::new(writer);
        let elem = writer
            .create_element("filter")
            .with_attribute(("type", self.as_str()));
        _ = match self {
            Self::Subtree(filter) => elem.write_inner_content(|writer| {
                writer
                    .get_mut()
                    .write_all(filter.as_bytes())
                    .map_err(|err| WriteError::Other(err.into()))
            })?,
            Self::XPath(select) => elem
                .with_attribute(("select", select.as_str()))
                .write_empty()?,
        };
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    inner: Box<str>,
}

impl ReadXml for Reply {
    #[tracing::instrument(skip(reader))]
    fn read_xml(reader: &mut NsReader<&[u8]>, start: &BytesStart<'_>) -> Result<Self, ReadError> {
        let end = start.to_end();
        let inner = reader.read_text(end.name())?.into();
        Ok(Self { inner })
    }
}

impl ReplyData for Reply {
    type Ok = Self;

    fn from_ok() -> Result<Self::Ok, Error> {
        Err(Error::EmptyRpcReply)
    }

    fn into_result(self) -> Result<Self::Ok, Error> {
        Ok(self)
    }
}

impl Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl AsRef<str> for Reply {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct Url {
    inner: Arc<UriStr>,
}

impl Url {
    fn try_new<S: AsRef<str>>(s: S, ctx: &Context) -> Result<Self, Error> {
        let url = UriStr::new(s.as_ref())?;
        ctx.server_capabilities()
            .iter()
            .filter_map(|capability| {
                if let Capability::Url(schemes) = capability {
                    Some(schemes.iter())
                } else {
                    None
                }
            })
            .flatten()
            .find(|&scheme| url.scheme_str() == scheme.as_ref())
            .ok_or_else(|| Error::UnsupportedUrlScheme(url.into()))
            .map(|_| Self { inner: url.into() })
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.inner.as_ref(), f)
    }
}

impl WriteXml for Url {
    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), WriteError> {
        _ = Writer::new(writer)
            .create_element("url")
            .write_text_content(BytesText::new(self.inner.as_str()))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    inner: Arc<str>,
}

impl Token {
    pub fn new<S: AsRef<str>>(token: S) -> Self {
        let inner = token.as_ref().into();
        Self { inner }
    }

    #[must_use]
    pub fn generate() -> Self {
        let inner = Arc::from(
            &*Uuid::new_v4()
                .urn()
                .encode_lower(&mut Uuid::encode_buffer()),
        );
        Self { inner }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use quick_xml::events::Event;

    #[test]
    fn reply_from_xml() {
        let reply = "<configuration><top/></configuration>";
        let expect = Reply {
            inner: reply.into(),
        };
        let msg = format!("<data>{reply}</data>");
        let mut reader = NsReader::from_str(msg.as_str());
        _ = reader.trim_text(true);
        if let Event::Start(start) = reader.read_event().unwrap() {
            assert_eq!(Reply::read_xml(&mut reader, &start).unwrap(), expect);
        } else {
            panic!("missing <data> tag")
        }
    }
}
