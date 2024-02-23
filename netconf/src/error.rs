use std::convert::Infallible;

use bytes::Bytes;
use iri_string::types::UriStr;
use tokio::sync::mpsc;

use crate::{
    capabilities::Requirements,
    message::{
        self,
        rpc::{self, operation::Datastore},
    },
};

// TODO: name non-source fields
/// `netconf` library error variants
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // Transport //
    #[cfg(feature = "ssh")]
    /// The underlying SSH transport encountered an error.
    #[error(transparent)]
    SshTransport(#[from] russh::Error),

    #[cfg(feature = "tls")]
    /// The underlying TLS transport encountered an error.
    #[error(transparent)]
    TlsTransport(#[from] tokio_rustls::rustls::Error),

    /// The underlying transport encountered an error.
    #[error("a transport error occurred: {0}")]
    Transport(#[from] std::io::Error),

    /// Failure to enqueue an inter-task message.
    #[error("failed to enqueue a message")]
    EnqueueMessage(#[from] mpsc::error::SendError<Bytes>),

    /// Failure to dequeue an inter-task message.
    #[error("failed to dequeue a message: send side is closed")]
    DequeueMessage,

    #[cfg(feature = "tls")]
    /// Invalid DNS name for certificate validation.
    #[error(transparent)]
    InvalidDnsName(#[from] rustls_pki_types::InvalidDnsNameError),

    // Session establishment //
    //
    /// User authentication failed.
    #[error("authentication failed for user {0}")]
    Authentication(String),

    /// Base protocol version negotiation failed.
    #[error("failed to negotiate a common base protocol version")]
    VersionNegotiation,

    // Session management //
    //
    /// A `message-id` collision was detected.
    #[error("encountered a 'message-id' collision. please file a bug report!")]
    MessageIdCollision(rpc::MessageId),

    /// Received a message with an unknown `message-id`.
    #[error("request with message-id '{0:?}' not found")]
    RequestNotFound(rpc::MessageId),

    /// Attempted to process an already completed request.
    #[error("attempted to poll for an already completed request")]
    RequestComplete,

    // Message encoding / serialization //
    //
    /// Message serialization failed.
    #[error(transparent)]
    WriteMessage(#[from] message::WriteError),

    // Message decoding / de-serialization //
    //
    /// Message de-serialization failed
    #[error(transparent)]
    ReadMessage(#[from] message::ReadError),

    // RPC request validation.
    //
    /// Attempted to perform `delete-config` operation targeting the `running` datastore.
    #[error("deleting the <running/> datastore is not permitted")]
    DeleteRunningConfig,

    /// Invalid `session-id`.
    #[error("invalid session-id: {0}")]
    InvalidSessionId(u32),

    /// Attempted to perform `kill-session` operation targeting the current session.
    #[error("kill-session operation targeting the current session is not permitted")]
    KillCurrentSession,

    /// Attempted to perform an unsupported operation.
    #[error("unsupported rpc operation '{0}' (requires {1})")]
    UnsupportedOperation(&'static str, Requirements),

    /// Attempted to set an unsupported operation parameter.
    #[error("unsupported parameter '{1}' for rpc operation '{0}' (requires {2})")]
    UnsupportedOperationParameter(&'static str, &'static str, Requirements),

    /// Attempted to set an operation parameter to an unsupported value.
    #[error("unsupported value '{2}' of parameter '{1}' for rpc operation '{0}' (requires {3})")]
    UnsupportedOperParameterValue(&'static str, &'static str, &'static str, Requirements),

    /// Attempted to perform an operation on an unsupported `source` datastore.
    #[error("unsupported source datastore '{0:?}' (requires {1})")]
    UnsupportedSource(Datastore, Requirements),

    /// Attempted to perform an operation on an unsupported `target` datastore.
    #[error("unsupported target datastore '{0:?}' (requires {1})")]
    UnsupportedTarget(Datastore, Requirements),

    /// Attempted to `lock` an unsupported datastore.
    #[error("unsupported lock target datastore '{0:?}' (requires {1})")]
    UnsupportedLockTarget(Datastore, Requirements),

    /// Unsupported URL scheme.
    #[error("unsupported scheme in url '{0}' (requires ':url:1.0' capability with corresponding 'scheme' parameter)")]
    UnsupportedUrlScheme(Box<UriStr>),

    /// Unsupported `filter` type.
    #[error("unsupported filter type '{0}' (requires {1})")]
    UnsupportedFilterType(&'static str, Requirements),

    /// Missing a required operation parameter.
    #[error("missing required parameter {param_name} for rpc operation {operation_name}")]
    MissingOperationParameter {
        /// RPC operation name.
        operation_name: &'static str,
        /// Required parameter name.
        param_name: &'static str,
    },

    /// Incompatible combination of operation parameters.
    #[error("incompatible parameter combination for operation '{0}': {}", .1.join(", "))]
    IncompatibleOperationParameters(&'static str, Vec<&'static str>),

    /// Failed to parse a URL
    #[error("failed to parse URI")]
    UrlParse(#[from] iri_string::validate::Error),

    // Protocol errors
    //
    /// RPC operation failure.
    #[error("received rpc-error reply: {0}")]
    RpcError(#[from] rpc::Errors),

    /// Empty `rpc-reply` when data was expected.
    #[error("unexpectedly empty rpc-reply")]
    EmptyRpcReply,

    #[cfg(feature = "junos")]
    /// Errors occurred during a `load-configuration` operation.
    #[error("load-configuration operation failed with {0} errors")]
    LoadConfiguration(usize),
}

impl Error {
    pub(crate) const fn missing_operation_parameter(
        operation_name: &'static str,
        param_name: &'static str,
    ) -> Self {
        Self::MissingOperationParameter {
            operation_name,
            param_name,
        }
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
