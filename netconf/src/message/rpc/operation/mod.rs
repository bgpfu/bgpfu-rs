use std::fmt::Debug;

use crate::message::{ReadXml, WriteXml};

pub trait Operation: Debug + WriteXml + Send + Sync {
    type ReplyData: Debug + ReadXml;
}

pub mod get_config;
#[doc(inline)]
pub use self::get_config::GetConfig;

pub(crate) mod close_session;
pub(crate) use self::close_session::CloseSession;
