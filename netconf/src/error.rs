use bytes::Bytes;
use tokio::sync::mpsc;

use crate::message::rpc;

/// `netconf` Error variants
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    SshTransport(#[from] russh::Error),

    #[error("authentication failed for user {0}")]
    Authentication(String),

    #[error("a transport error occurred: {0}")]
    Transport(#[from] std::io::Error),

    #[error("failed to enqueue a message")]
    EnqueueMessage(#[from] mpsc::error::SendError<Bytes>),

    #[error("failed to dequeue a message: {0}")]
    DequeueMessage(&'static str),

    #[error("failed to utf-8 encode message")]
    EncodeMessage(#[from] std::string::FromUtf8Error),

    #[error("failed to decode utf-8")]
    DecodeMessage(#[from] std::str::Utf8Error),

    #[error("failed to parse xml document: {0:?}")]
    XmlParse(#[from] Option<quick_xml::Error>),

    #[error("missing '{1}' element while parsing '{0}' message")]
    MissingElement(&'static str, &'static str),

    #[error("missing '{1}' attribute while parsing '{0}' message")]
    MissingAttribute(&'static str, &'static str),

    #[error("message-id attribute missing in rpc-reply")]
    NoMessageId,

    #[error("message-id mis-match between parse phases. please file a bug report!")]
    MessageIdMismatch(rpc::MessageId, rpc::MessageId),

    #[error("failed to parse message-id")]
    MessageIdParse(#[from] std::num::ParseIntError),

    #[error("missing common mandatory capabilities")]
    BaseCapability,

    #[error("encountered a 'message-id' collision. please file a bug report!")]
    MessageIdCollision(rpc::MessageId),

    #[error("request with message-id '{0:?}' not found")]
    RequestNotFound(rpc::MessageId),

    #[error("attempted to poll for an already completed request")]
    RequestComplete,

    #[error("failed to serialize rpc request")]
    RpcRequestSerialization(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("failed to deserialize rpc-reply data")]
    RpcReplyDeserialization(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),

    #[error("received rpc-error reply: {0}")]
    RpcError(#[from] rpc::RpcError),
}
