use std::fmt::Debug;

use crate::message::{ReadXml, ToXml};

pub trait Operation: Debug + ToXml + Send + Sync {
    type ReplyData: Debug + ReadXml;
}

pub mod get_config {
    use std::{fmt, io::Write};

    use quick_xml::{events::BytesStart, NsReader, Writer};

    use crate::Error;

    use super::{Operation, ReadXml, ToXml};

    #[derive(Debug, Default, Clone)]
    pub struct GetConfig {
        source: Source,
        filter: Option<String>,
    }

    impl GetConfig {
        #[must_use]
        pub const fn new(source: Source, filter: Option<String>) -> Self {
            Self { source, filter }
        }
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
        inner: Box<str>,
    }

    impl ReadXml for Reply {
        type Error = Error;

        #[tracing::instrument(skip(reader))]
        fn read_xml(
            reader: &mut NsReader<&[u8]>,
            start: &BytesStart<'_>,
        ) -> Result<Self, Self::Error> {
            let end = start.to_end();
            let inner = reader.read_text(end.name())?.into();
            Ok(Self { inner })
        }
    }

    impl fmt::Display for Reply {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.inner.fmt(f)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        use quick_xml::events::Event;

        #[test]
        fn default_request_to_xml() {
            let req = GetConfig::default();
            let expect = "<get-config><source><running/></source></get-config>]]>]]>";
            assert_eq!(req.to_xml().unwrap(), expect);
        }

        #[test]
        fn reply_from_xml() {
            let reply = "<configuration><top/></configuration>";
            let expect = Reply {
                inner: reply.into(),
            };
            let msg = format!("<data>{reply}</data>");
            let mut reader = NsReader::from_str(msg.as_str());
            _ = reader.trim_text(true);
            if let Event::Start(start) = reader.read_event().unwrap() {
                assert_eq!(Reply::read_xml(&mut reader, &start).unwrap(), expect);
            } else {
                panic!("missing <data> tag")
            }
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
            let expect = "<close-session/>]]>]]>";
            assert_eq!(req.to_xml().unwrap(), expect);
        }
    }
}
