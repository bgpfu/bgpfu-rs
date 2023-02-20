use std::io::Write;

use anyhow::Result;
use clap::Clap;
use irrc::{IrrClient, Query, ResponseItem, RpslObjectClass};

use super::Dispatch;

#[derive(Clap, Debug)]
pub struct Whois {
    #[clap(from_global)]
    addr: String,

    /// RSPL object name to search for.
    name: String,
}

impl<W: Write> Dispatch<W> for Whois {
    fn dispatch(&self, w: &mut W) -> Result<()> {
        log::info!("searching for '{}'", self.name);
        IrrClient::new(&self.addr)
            .connect()?
            .pipeline()
            .push(Query::RpslObject(
                RpslObjectClass::AutNum,
                self.name.clone(),
            ))?
            .responses()
            .filter_map(|item| {
                item.map_err(|err| log::error!("failed to parse object: {}", err))
                    .ok()
            })
            .try_for_each(|item: ResponseItem<String>| writeln!(w, "{}", item.content()))?;
        Ok(())
    }
}
