use std::fmt;

use anyhow::{anyhow, Result};

use super::{
    AsSetExpr, FilterExpr, FilterSetExpr, FilterTerm, NamedPrefixSet, PrefixSetExpr, RouteSetExpr,
    SetNameComp,
};
use crate::query::Resolver;

macro_rules! debug_substitution {
    ( $node:ty ) => {
        log::debug!(concat!("substituting AST node '", stringify!($node),))
    };
}

pub trait Substitute: Sized {
    fn substitute(&self, resolver: &Resolver) -> Result<Self>;
}

impl Substitute for FilterExpr {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(FilterExpr);
        match self {
            Self::Unit(term) => Ok(Self::Unit(term.substitute(resolver)?)),
            Self::Not(term) => Ok(Self::Not(term.substitute(resolver)?)),
            Self::And(lhs, rhs) => Ok(Self::And(
                lhs.substitute(resolver)?,
                rhs.substitute(resolver)?,
            )),
            Self::Or(lhs, rhs) => Ok(Self::Or(
                lhs.substitute(resolver)?,
                rhs.substitute(resolver)?,
            )),
        }
    }
}

impl Substitute for FilterTerm {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(FilterTerm);
        match self {
            Self::Literal(set_expr, op) => Ok(Self::Literal(set_expr.substitute(resolver)?, *op)),
            Self::Named(fltr_set_expr) => Ok(Self::Named(fltr_set_expr.substitute(resolver)?)),
            Self::Expr(expr) => Ok(Self::Expr(Box::new(expr.substitute(resolver)?))),
        }
    }
}

impl Substitute for PrefixSetExpr {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(PrefixSetExpr);
        match self {
            literal @ Self::Literal(_) => Ok(literal.clone()),
            Self::Named(set) => Ok(Self::Named(set.substitute(resolver)?)),
        }
    }
}

impl Substitute for FilterSetExpr {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(FilterSetExpr);
        match self {
            Self::Pending(components) => Ok(Self::Ready(
                components
                    .iter()
                    .map(|component| Ok(component.substitute(resolver)?.to_string()))
                    .collect::<Result<Vec<_>>>()?
                    .join(":")
                    .parse()?,
            )),
            ready @ Self::Ready(_) => Ok(ready.clone()),
        }
    }
}

impl Substitute for NamedPrefixSet {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(NamedPrefixSet);
        match self {
            Self::PeerAs => {
                if let Some(peeras) = resolver.peeras() {
                    Ok(Self::AutNum(*peeras))
                } else {
                    Err(anyhow!(
                        "no 'PeerAS' value available to perform substitution. See '--peeras' option."
                    ))
                }
            }
            Self::RouteSet(set_expr) => Ok(Self::RouteSet(set_expr.substitute(resolver)?)),
            Self::AsSet(set_expr) => Ok(Self::AsSet(set_expr.substitute(resolver)?)),
            _ => Ok(self.clone()),
        }
    }
}

impl Substitute for RouteSetExpr {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(RouteSetExpr);
        match self {
            Self::Pending(components) => Ok(Self::Ready(
                components
                    .iter()
                    .map(|component| Ok(component.substitute(resolver)?.to_string()))
                    .collect::<Result<Vec<_>>>()?
                    .join(":")
                    .parse()?,
            )),
            ready @ Self::Ready(_) => Ok(ready.clone()),
        }
    }
}

impl Substitute for AsSetExpr {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(AsSetExpr);
        match self {
            Self::Pending(components) => Ok(Self::Ready(
                components
                    .iter()
                    .map(|component| Ok(component.substitute(resolver)?.to_string()))
                    .collect::<Result<Vec<_>>>()?
                    .join(":")
                    .parse()?,
            )),
            ready @ Self::Ready(_) => Ok(ready.clone()),
        }
    }
}

impl Substitute for SetNameComp {
    fn substitute(&self, resolver: &Resolver) -> Result<Self> {
        debug_substitution!(SetNameComp);
        match self {
            Self::PeerAs => {
                if let Some(peeras) = resolver.peeras() {
                    Ok(Self::AutNum(*peeras))
                } else {
                    Err(anyhow!(
                        "no 'PeerAS' value available to perform substitution. See '--peeras' option."
                    ))
                }
            }
            _ => Ok(self.clone()),
        }
    }
}

impl fmt::Display for SetNameComp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::AutNum(autnum) => autnum.fmt(f),
            Self::PeerAs => write!(f, "PeerAS"),
            Self::Name(name) => name.fmt(f),
        }
    }
}
