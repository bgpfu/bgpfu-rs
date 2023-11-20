use std::fmt::Debug;

use crate::message::{FromXml, ToXml};

pub trait Operation: Debug + ToXml + Send + Sync {
    type ReplyData: Debug + FromXml;
}

pub mod get_config {
    use std::{fmt, io::Write};

    use quick_xml::{events::Event, Reader, Writer};

    use crate::Error;

    use super::{FromXml, Operation, ToXml};

    #[derive(Debug, Default, Clone)]
    pub struct GetConfig {
        source: Source,
        filter: Option<String>,
    }

    impl Operation for GetConfig {
        type ReplyData = Reply;
    }

    impl ToXml for GetConfig {
        type Error = Error;

        fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
            _ = Writer::new(writer)
                .create_element("get-config")
                .write_inner_content(|writer| {
                    _ = writer
                        .create_element("source")
                        .write_inner_content(|writer| self.source.write_xml(writer.get_mut()))?;
                    if let Some(ref filter) = self.filter {
                        _ = writer
                            .create_element("filter")
                            .write_inner_content(|writer| {
                                writer
                                    .get_mut()
                                    .write_all(filter.as_bytes())
                                    .map_err(|err| Error::RpcRequestSerialization(err.into()))
                            })?;
                    };
                    Ok::<_, Self::Error>(())
                })?;
            Ok(())
        }
    }

    #[derive(Debug, Default, Copy, Clone)]
    pub enum Source {
        #[default]
        Running,
    }

    impl ToXml for Source {
        type Error = Error;

        fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
            let mut writer = Writer::new(writer);
            _ = match self {
                Self::Running => writer.create_element("running").write_empty()?,
            };
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Reply {
        configuration: Box<str>,
    }

    impl FromXml for Reply {
        type Error = Error;

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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn default_request_to_xml() {
            let req = GetConfig::default();
            let expect = "<get-config><source><running/></source></get-config>";
            assert_eq!(req.to_xml().unwrap(), expect);
        }

        #[test]
        fn reply_from_xml() {
            let reply = "<configuration><top/></configuration>";
            let expect = Reply {
                configuration: "<top/>".into(),
            };
            assert_eq!(Reply::from_xml(reply).unwrap(), expect);
        }
    }
}

pub mod close_session {
    use std::io::Write;

    use quick_xml::Writer;

    use super::{super::Empty, Operation, ToXml};
    use crate::Error;

    #[derive(Debug, Default, Clone, Copy)]
    pub struct CloseSession;

    impl Operation for CloseSession {
        type ReplyData = Empty;
    }

    impl ToXml for CloseSession {
        type Error = Error;

        fn write_xml<W: Write>(&self, writer: &mut W) -> Result<(), Self::Error> {
            _ = Writer::new(writer)
                .create_element("close-session")
                .write_empty()?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_to_xml() {
            let req = CloseSession;
            let expect = "<close-session/>";
            assert_eq!(req.to_xml().unwrap(), expect);
        }
    }
}
