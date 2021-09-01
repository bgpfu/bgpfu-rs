use std::io::Write;

use anyhow::Result;
use clap::Clap;

mod peval;

use self::peval::Peval;

/// Dispatch behaviour for `bgpfu` subcommands.
pub trait Dispatch<W: Write> {
    /// Execute sub-command.
    fn dispatch(&self, writer: &mut W) -> Result<()>;
}

#[allow(missing_docs)]
#[derive(Clap, Debug)]
pub enum Cmd {
    /// Evaluate an RPSL filter expression.
    Peval(Peval),
}

impl<W: Write> Dispatch<W> for Cmd {
    fn dispatch(&self, w: &mut W) -> Result<()> {
        match self {
            Self::Peval(peval) => peval.dispatch(w),
        }
    }
}
