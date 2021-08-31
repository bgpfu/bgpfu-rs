use std::io::Write;

use anyhow::Result;
use lazy_static::lazy_static;
use prefixset::{Ipv4Prefix, Ipv6Prefix, PrefixSet};
use strum::{
    AsRefStr, Display, EnumIter, EnumMessage, EnumString, EnumVariantNames, IntoEnumIterator,
};

use crate::query::PrefixSetPair;

mod plain;

use self::plain::{Plain, PlainRanges};

/// Static dispatch for output formats.
#[derive(AsRefStr, Debug, Display, EnumIter, EnumMessage, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum Format {
    /// Write plain prefixes using [`Display`][std::fmt::Display].
    #[strum(message = "Plain text list of prefixes.")]
    Plain,
    /// Write plain prefix ranges using [`Display`][std::fmt::Display].
    #[strum(message = "Plain text list of prefix ranges.")]
    PlainRanges,
}

impl Format {
    /// Write prefix sets using selected [`Formatter`].
    pub fn write_prefix_sets<W: Write>(&self, sets: &PrefixSetPair, w: &mut W) -> Result<()> {
        let formatter: Box<dyn Formatter<W>> = match self {
            Self::Plain => Box::new(Plain::default()),
            Self::PlainRanges => Box::new(PlainRanges::default()),
        };
        formatter.write_prefix_sets(sets, w)
    }
}

impl Default for Format {
    fn default() -> Self {
        Self::Plain
    }
}

lazy_static! {
    /// Auto-generated long help for format options.
    pub static ref FORMAT_HELP: String = {
        let mut help = "Output format.\n\nThe following output formats are available:\n" .to_string();
        help.extend(
            Format::iter().map(|variant| format!(
                "  {:16} {}\n",
                variant,
                variant.get_message().unwrap()
            ))
        );
        help.push('\n');
        help
    };
}

trait Formatter<W: Write> {
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
