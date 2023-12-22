use std::{
    fmt::{self, Debug, Display},
    io::Write,
    sync::Arc,
};

use crate::{
    capabilities::Capability,
    message::{ReadXml, WriteXml},
    session::Context,
    Error,
};

use iri_string::types::UriStr;
use quick_xml::Writer;

pub trait Operation: Debug + WriteXml + Send + Sync + Sized {
    type Builder<'a>: Builder<'a, Self>;
    type ReplyData: ReplyData;

    fn new<'a, F>(ctx: &'a Context, build_fn: F) -> Result<Self, Error>
    where
        F: Fn(Self::Builder<'a>) -> Result<Self, Error>,
    {
        Self::Builder::new(ctx).build(build_fn)
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

pub mod get;
#[doc(inline)]
pub use self::get::{Get, GetConfig};

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
        let required_capability = match self {
            Self::Running => None,
            Self::Candidate => Some(Capability::Candidate),
            Self::Startup => Some(Capability::Startup),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedSource(self, capability))
                }
            },
        )
    }

    fn try_as_target(self, ctx: &Context) -> Result<Self, Error> {
        let required_capability = match self {
            Self::Running => Some(Capability::WritableRunning),
            Self::Candidate => Some(Capability::Candidate),
            Self::Startup => Some(Capability::Startup),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedTarget(self, capability))
                }
            },
        )
    }

    fn try_as_lock_target(self, ctx: &Context) -> Result<Self, Error> {
        let required_capability = match self {
            Self::Running => None,
            Self::Candidate => Some(Capability::Candidate),
            Self::Startup => Some(Capability::Startup),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedLockTarget(self, capability))
                }
            },
        )
    }
}

impl WriteXml for Datastore {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
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
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        match self {
            Self::Datastore(datastore) => datastore.write_xml(writer)?,
            Self::Config(config) => {
                _ = Writer::new(writer)
                    .create_element("config")
                    .write_inner_content(|writer| {
                        writer
                            .get_mut()
                            .write_all(config.as_bytes())
                            .map_err(|err| Error::RpcRequestSerialization(err.into()))
                    })?;
            }
            Self::Url(url) => url.write_xml(writer)?,
        };
        Ok(())
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
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        _ = Writer::new(writer)
            .create_element("url")
            .write_inner_content(|writer| {
                write!(writer.get_mut(), "{self}")?;
                Ok::<_, Error>(())
            })?;
        Ok(())
    }
}
