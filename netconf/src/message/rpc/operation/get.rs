use std::io::Write;

use quick_xml::Writer;

use crate::{capabilities::Requirements, message::WriteError, session::Context, Error};

use super::{Filter, Operation, Reply, WriteXml};

#[derive(Debug, Default, Clone)]
pub struct Get {
    filter: Option<Filter>,
}

impl Operation for Get {
    const NAME: &'static str = "get";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder;
    type ReplyData = Reply;
}

impl WriteXml for Get {
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                if let Some(ref filter) = self.filter {
                    filter.write_xml(writer)?;
                };
                Ok(())
            })
            .map(|_| ())
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct Builder {
    filter: Option<Filter>,
}

impl Builder {
    pub fn filter(mut self, filter: Option<Filter>) -> Self {
        self.filter = filter;
        self
    }
}

impl super::Builder<'_, Get> for Builder {
    fn new(_: &Context) -> Self {
        Self { filter: None }
    }

    fn finish(self) -> Result<Get, Error> {
        Ok(Get {
            filter: self.filter,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{
        rpc::{MessageId, Request},
        ClientMsg,
    };

    #[test]
    fn default_request_to_xml() {
        let req = Request {
            message_id: MessageId(101),
            operation: Get::default(),
        };
        let expect = r#"<rpc message-id="101"><get></get></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
