use super::rpc;

#[derive(Debug, thiserror::Error)]
#[error("failed to deserialize message content from XML")]
pub enum Read {
    Xml(#[from] quick_xml::Error),

    /// UTF-8 decoding failed.
    #[error("failed to decode utf-8")]
    DecodeMessage(#[from] std::str::Utf8Error),

    /// Failed to parse a [`MessageId`][rpc::MessageId].
    #[error("failed to parse message-id")]
    MessageIdParse(#[source] std::num::ParseIntError),

    #[error("failed to parse session-id")]
    SessionIdParse(#[source] std::num::ParseIntError),

    #[error("unexpected event while parsing xml: {0:?}")]
    UnexpectedXmlEvent(quick_xml::events::Event<'static>),

    #[error("message-id attribute missing in rpc-reply")]
    NoMessageId,

    #[error("message-id mis-match between parse phases. please file a bug report!")]
    MessageIdMismatch {
        initial: rpc::MessageId,
        new: rpc::MessageId,
    },

    #[error("missing '{element}' element while parsing '{msg_type}' message")]
    MissingElement {
        msg_type: &'static str,
        element: &'static str,
    },

    #[error("encountered an unknown rpc-error error-type: {0}")]
    UnknownErrorType(String),

    #[error("encountered an unknown rpc-error error-tag: {0}")]
    UnknownErrorTag(String),

    #[error("encountered an unknown rpc-error error-severity: {0}")]
    UnknownErrorSeverity(String),

    #[error("encountered an unknown rpc-error error-info type: {0}")]
    UnknownErrorInfo(String),

    #[error("failed to parse capability URI")]
    ParseCapability(#[from] iri_string::validate::Error),

    Other(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl Read {
    pub(super) const fn missing_element(msg_type: &'static str, element: &'static str) -> Self {
        Self::MissingElement { msg_type, element }
    }

    pub(super) const fn message_id_mismatch(initial: rpc::MessageId, new: rpc::MessageId) -> Self {
        Self::MessageIdMismatch { initial, new }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to serialize message content as XML")]
pub enum Write {
    Xml(#[from] quick_xml::Error),

    /// UTF-8 encoding failed.
    #[error("failed to utf-8 encode message")]
    EncodeMessage(#[from] std::string::FromUtf8Error),

    Other(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
}
