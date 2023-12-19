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

pub mod get_config;
#[doc(inline)]
pub use self::get_config::GetConfig;

pub mod edit_config;
#[doc(inline)]
pub use self::edit_config::EditConfig;

pub(crate) mod close_session;
pub(crate) use self::close_session::CloseSession;

// TODO:
// implement remaining base rpc operations:
// - get
// - edit-config
// - copy-config
// - delete-config
// - lock
// - unlock
// - kill-session

#[derive(Debug, Default, Copy, Clone)]
pub enum Datastore {
    #[default]
    Running,
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
