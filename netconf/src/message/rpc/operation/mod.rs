use std::{fmt::Debug, io::Write};

use crate::{
    message::{ReadXml, WriteXml},
    Error,
};

use quick_xml::Writer;

pub trait Operation: Debug + WriteXml + Send + Sync {
    type ReplyData: Debug + ReadXml;
}

pub mod get_config;
#[doc(inline)]
pub use self::get_config::GetConfig;

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
