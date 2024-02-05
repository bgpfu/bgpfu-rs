use std::{fmt::Debug, io::Write, marker::PhantomData};

use quick_xml::Writer;

use crate::{
    capabilities::Requirements,
    message::{ReadXml, WriteError},
    session::Context,
    Error,
};

use super::{params::Required, DataReply, Datastore, Filter, Operation, WriteXml};

#[derive(Debug, Clone)]
pub struct GetConfig<D> {
    source: Datastore,
    filter: Option<Filter>,
    _reply: PhantomData<D>,
}

impl<D> Default for GetConfig<D> {
    fn default() -> Self {
        Self {
            source: Datastore::default(),
            filter: None,
            _reply: PhantomData,
        }
    }
}

impl<D> Operation for GetConfig<D>
where
    D: ReadXml + Debug + Send + Sync,
{
    const NAME: &'static str = "get-config";
    const REQUIRED_CAPABILITIES: Requirements = Requirements::None;

    type Builder<'a> = Builder<'a>;
    type Reply = DataReply<D>;
}

impl<D> WriteXml for GetConfig<D>
where
    D: ReadXml + Debug + Send + Sync,
{
    fn write_xml<W: Write>(&self, writer: &mut Writer<W>) -> Result<(), WriteError> {
        writer
            .create_element(Self::NAME)
            .write_inner_content(|writer| {
                _ = writer
                    .create_element("source")
                    .write_inner_content(|writer| self.source.write_xml(writer))?;
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
pub struct Builder<'a> {
    ctx: &'a Context,
    source: Required<Datastore>,
    filter: Option<Filter>,
}

impl Builder<'_> {
    pub fn source(mut self, source: Datastore) -> Result<Self, Error> {
        source.try_as_source(self.ctx).map(|source| {
            self.source.set(source);
            self
        })
    }

    pub fn filter(mut self, filter: Option<Filter>) -> Result<Self, Error> {
        self.filter = filter.map(|filter| filter.try_use(self.ctx)).transpose()?;
        Ok(self)
    }
}

impl<'a, D> super::Builder<'a, GetConfig<D>> for Builder<'a>
where
    D: ReadXml + Debug + Send + Sync,
{
    fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            source: Required::init(),
            filter: None,
        }
    }

    fn finish(self) -> Result<GetConfig<D>, Error> {
        Ok(GetConfig {
            source: self.source.require::<GetConfig<D>>("source")?,
            filter: self.filter,
            _reply: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{
        rpc::{operation::Opaque, MessageId, Request},
        ClientMsg,
    };

    #[test]
    fn default_request_to_xml() {
        let req: Request<GetConfig<Opaque>> = Request {
            message_id: MessageId(101),
            operation: GetConfig::default(),
        };
        let expect = r#"<rpc message-id="101"><get-config><source><running/></source></get-config></rpc>]]>]]>"#;
        assert_eq!(req.to_xml().unwrap(), expect);
    }
}
