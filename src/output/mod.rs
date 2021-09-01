use std::io::Write;

use anyhow::Result;
use clap::Clap;
use prefixset::{Ipv4Prefix, Ipv6Prefix, PrefixSet};

use crate::query::PrefixSetPair;

mod cisco_ios;
mod plain;

use self::{
    cisco_ios::CiscoIos,
    plain::{Plain, PlainRanges},
};

#[derive(Clap, Debug)]
#[clap(subcommand_placeholder("FORMAT", "FORMATS"))]
pub enum Format {
    /// Output plain text list of prefixes.
    #[clap(short_flag = 'P', long_flag = "plain")]
    Plain(Plain),
    /// Output plain text list of prefix ranges.
    #[clap(short_flag = 'R', long_flag = "plain-ranges")]
    PlainRanges(PlainRanges),
    /// Output Cisco IOS Classic/XE ip/ipv6 prefix-lists.
    #[clap(short_flag = 'C', long_flag = "cisco-ios")]
    CiscoIos(CiscoIos),
}

impl Format {
    fn as_formatter<W: Write>(&self) -> &dyn Formatter<W> {
        match self {
            Self::Plain(f) => f,
            Self::PlainRanges(f) => f,
            Self::CiscoIos(f) => f,
        }
    }
}

impl<W: Write> Formatter<W> for Format {
    fn write_prefix_set_ipv4(&self, set: &PrefixSet<Ipv4Prefix>, w: &mut W) -> Result<()> {
        self.as_formatter().write_prefix_set_ipv4(set, w)
    }

    fn write_prefix_set_ipv6(&self, set: &PrefixSet<Ipv6Prefix>, w: &mut W) -> Result<()> {
        self.as_formatter().write_prefix_set_ipv6(set, w)
    }
}

pub trait Formatter<W: Write> {
    fn write_prefix_set_ipv4(&self, set: &PrefixSet<Ipv4Prefix>, w: &mut W) -> Result<()>;

    fn write_prefix_set_ipv6(&self, set: &PrefixSet<Ipv6Prefix>, w: &mut W) -> Result<()>;

    fn write_prefix_sets(&self, sets: &PrefixSetPair, w: &mut W) -> Result<()> {
        if let (Some(set), _) = sets {
            self.write_prefix_set_ipv4(set, w)?;
        }
        if let (_, Some(set)) = sets {
            self.write_prefix_set_ipv6(set, w)?;
        }
        Ok(())
    }
}
