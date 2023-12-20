use std::{fmt::Debug, io::Write};

use crate::{
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

pub(crate) mod close_session;
pub(crate) use self::close_session::CloseSession;

#[derive(Debug, Default, Copy, Clone)]
pub enum Datastore {
    #[default]
    Running,
}

impl Datastore {
    const fn try_as_source(self, _: &Context) -> Result<Self, Error> {
        match self {
            Self::Running => Ok(self),
        }
    }

    fn try_as_target(self, ctx: &Context) -> Result<Self, Error> {
        todo!()
    }

    fn try_as_lock_target(self, ctx: &Context) -> Result<Self, Error> {
        todo!()
    }
}

impl WriteXml for Datastore {
    type Error = Error;

    fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
        let mut writer = Writer::new(writer);
        _ = match self {
            Self::Running => writer.create_element("running").write_empty()?,
        };
        Ok(())
    }
}
