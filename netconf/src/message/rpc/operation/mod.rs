use std::{fmt::Debug, io::Write};

use crate::{
    capabilities::Capability,
    message::{ReadXml, WriteXml},
    session::Context,
    Error,
};

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

pub(crate) mod close_session;
pub(crate) use self::close_session::CloseSession;

#[derive(Debug, Default, Copy, Clone)]
pub enum Datastore {
    #[default]
    Running,
    Candidate,
}

impl Datastore {
    fn try_as_source(self, ctx: &Context) -> Result<Self, Error> {
        let required_capability = match self {
            Self::Running => None,
            Self::Candidate => Some(Capability::Candidate),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedSource(self, Some(capability)))
                }
            },
        )
    }

    fn try_as_target(self, ctx: &Context) -> Result<Self, Error> {
        let required_capability = match self {
            Self::Running => Some(Capability::WritableRunning),
            Self::Candidate => Some(Capability::Candidate),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedTarget(self, Some(capability)))
                }
            },
        )
    }

    fn try_as_lock_target(self, ctx: &Context) -> Result<Self, Error> {
        let required_capability = match self {
            Self::Running => None,
            Self::Candidate => Some(Capability::Candidate),
        };
        required_capability.map_or_else(
            || Ok(self),
            |capability| {
                if ctx.server_capabilities().contains(&capability) {
                    Ok(self)
                } else {
                    Err(Error::UnsupportedLockTarget(self, Some(capability)))
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
        };
        Ok(())
    }
}
