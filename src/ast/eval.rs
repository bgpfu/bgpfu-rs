use anyhow::{anyhow, Result};
use ipnet::IpNet;
use itertools::{Either, Itertools};
use num::One;
use prefixset::{IpPrefix, IpPrefixRange, Ipv4Prefix, Ipv6Prefix, PrefixSet};

use super::{
    FilterExpr, FilterTerm, LiteralPrefixSetEntry, NamedPrefixSet, PrefixOp, PrefixSetExpr,
    PrefixSetOp,
};
use crate::query::{PrefixSetPair, Resolver};

pub trait Evaluate {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair>;
}

impl Evaluate for FilterExpr {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        match self {
            Self::Unit(term) => term.eval(resolver),
            Self::Not(term) => {
                let (ipv4, ipv6) = term.eval(resolver)?;
                Ok((ipv4.map(|set| !set), ipv6.map(|set| !set)))
            }
            Self::And(lhs, rhs) => {
                let (lhs_ipv4, lhs_ipv6) = lhs.eval(resolver)?;
                let (rhs_ipv4, rhs_ipv6) = rhs.eval(resolver)?;
                let ipv4 = match (lhs_ipv4, rhs_ipv4) {
                    (Some(lhs), Some(rhs)) => Some(lhs & rhs),
                    (None, None) => None,
                    _ => return Err(anyhow!("failed to take intersection of sets")),
                };
                let ipv6 = match (lhs_ipv6, rhs_ipv6) {
                    (Some(lhs), Some(rhs)) => Some(lhs & rhs),
                    (None, None) => None,
                    _ => return Err(anyhow!("failed to take intersection of sets")),
                };
                Ok((ipv4, ipv6))
            }
            Self::Or(lhs, rhs) => {
                let (lhs_ipv4, lhs_ipv6) = lhs.eval(resolver)?;
                let (rhs_ipv4, rhs_ipv6) = rhs.eval(resolver)?;
                let ipv4 = match (lhs_ipv4, rhs_ipv4) {
                    (Some(lhs), Some(rhs)) => Some(lhs | rhs),
                    (None, None) => None,
                    _ => return Err(anyhow!("failed to take union of sets")),
                };
                let ipv6 = match (lhs_ipv6, rhs_ipv6) {
                    (Some(lhs), Some(rhs)) => Some(lhs | rhs),
                    (None, None) => None,
                    _ => return Err(anyhow!("failed to take union of sets")),
                };
                Ok((ipv4, ipv6))
            }
        }
    }
}

impl Evaluate for FilterTerm {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        match self {
            Self::Literal(set_expr, op) => Ok(op.apply(set_expr.eval(resolver)?)),
            // TODO
            Self::Named(_) => Err(anyhow!("named filter-sets not yet implemented")),
            Self::Expr(expr) => expr.eval(resolver),
        }
    }
}

impl PrefixSetOp {
    fn apply(&self, pair: PrefixSetPair) -> PrefixSetPair {
        let (ipv4, ipv6) = pair;
        (
            ipv4.map(|set| self.apply_map(set)),
            ipv6.map(|set| self.apply_map(set)),
        )
    }

    fn apply_map<P: IpPrefix>(&self, set: PrefixSet<P>) -> PrefixSet<P> {
        set.ranges()
            .filter_map(|range| {
                self.apply_each(range)
                    .map_err(|err| log::warn!("failed to apply prefix range operator: {}", err))
                    .ok()
            })
            .collect()
    }

    fn apply_each<P: IpPrefix>(
        &self,
        range: IpPrefixRange<P>,
    ) -> Result<IpPrefixRange<P>, prefixset::Error> {
        let lower = match self {
            Self::None => return Ok(range),
            Self::LessExcl => *range.range().start() + 1,
            Self::LessIncl => *range.range().start(),
        };
        let upper = P::MAX_LENGTH;
        IpPrefixRange::new(*range.base(), lower, upper)
    }
}

impl Evaluate for PrefixSetExpr {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        match self {
            Self::Literal(entries) => {
                let sets = entries
                    .into_iter()
                    .filter_map(|entry| {
                        entry
                            .into_prefix_range()
                            .map_err(|err| {
                                log::warn!("failed to apply prefix range operator: {}", err)
                            })
                            .ok()
                    })
                    .partition_map(|range| range);
                Ok(resolver.filter_pair(sets))
            }
            Self::Named(set) => set.eval(resolver),
        }
    }
}

impl LiteralPrefixSetEntry {
    fn into_prefix_range(
        self,
    ) -> Result<Either<IpPrefixRange<Ipv4Prefix>, IpPrefixRange<Ipv6Prefix>>> {
        match self.prefix {
            IpNet::V4(prefix) => Ok(Either::Left(self.op.apply(prefix.into())?)),
            IpNet::V6(prefix) => Ok(Either::Right(self.op.apply(prefix.into())?)),
        }
    }
}

impl PrefixOp {
    fn apply<P: IpPrefix>(&self, prefix: P) -> Result<IpPrefixRange<P>, prefixset::Error> {
        let (lower, upper) = match self {
            Self::None => (prefix.length(), prefix.length()),
            Self::LessExcl => (prefix.length() + 1, P::MAX_LENGTH),
            Self::LessIncl => (prefix.length(), P::MAX_LENGTH),
            Self::Exact(length) => (*length, *length),
            Self::Range(upper, lower) => (*upper, *lower),
        };
        IpPrefixRange::new(prefix, lower, upper)
    }
}

impl Evaluate for NamedPrefixSet {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        match self {
            Self::Any => Ok((Some(PrefixSet::one()), Some(PrefixSet::one()))),
            // TODO
            Self::PeerAs => unimplemented!("PeerAs substitution not yet implemented"),
            Self::RouteSet(route_set) => resolver.job(route_set),
            Self::AsSet(as_set) => resolver.job(as_set),
            Self::AutNum(autnum) => resolver.job(autnum),
        }
    }
}
