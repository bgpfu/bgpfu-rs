use std::fmt::Debug;

use serde::Serialize;

use crate::message::FromXml;

pub trait Operation: Debug + Send + Sync {
    type RequestData: Debug + Serialize + Send + Sync;
    type ReplyData: Debug + FromXml;
}

pub mod get_config {
    use std::fmt;

    use quick_xml::{events::Event, Reader};
    use serde::{Deserialize, Serialize};

    use super::{FromXml, Operation};

    #[derive(Debug, Clone, Copy)]
    pub struct GetConfig;

    impl Operation for GetConfig {
        type RequestData = Request;
        type ReplyData = Reply;
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct Request {
        get_config: RequestInner,
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct RequestInner {
        source: Source,
        #[serde(skip_serializing_if = "Option::is_none")]
        filter: Option<String>,
    }

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
    pub struct Source {
        #[serde(rename = "$value")]
        inner: SourceInner,
    }

    #[derive(Debug, Default, Copy, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum SourceInner {
        #[default]
        Running,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Reply {
        configuration: Box<str>,
    }

    impl FromXml for Reply {
        type Error = crate::Error;

        fn from_xml<S>(input: S) -> Result<Self, Self::Error>
        where
            S: AsRef<str> + std::fmt::Debug,
        {
            let mut reader = Reader::from_str(input.as_ref());
            _ = reader.trim_text(true);
            tracing::debug!("expecting <configuration>");
            let configuration = match reader.read_event()? {
                Event::Start(tag) if tag.name().as_ref() == b"configuration" => {
                    let span = reader.read_text(tag.to_end().name())?;
                    span.as_ref().into()
                }
                event => {
                    tracing::error!(?event, "unexpected xml event");
                    return Err(crate::Error::XmlParse(None));
                }
            };
            tracing::debug!("expecting eof");
            if matches!(reader.read_event()?, Event::Eof) {
                tracing::debug!(?configuration);
                Ok(Self { configuration })
            } else {
                Err(crate::Error::XmlParse(None))
            }
        }
    }

    impl fmt::Display for Reply {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.configuration.fmt(f)
        }
    }
}

pub mod close_session {
    use serde::{Deserialize, Serialize};

    use super::{super::Empty, Operation};

    #[derive(Debug, Clone, Copy)]
    pub struct CloseSession;

    impl Operation for CloseSession {
        type RequestData = Request;
        type ReplyData = Empty;
    }

    #[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub struct Request {
        close_session: (),
    }
}
