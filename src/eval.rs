use std::cmp::{max, min};

use anyhow::{anyhow, Result};
use ipnet::IpNet;
use itertools::{Either, Itertools};
use num::One;
use prefixset::{IpPrefix, IpPrefixRange, Ipv4Prefix, Ipv6Prefix, PrefixSet};
use rpsl::{
    expr::{FilterExpr, FilterTerm, NamedPrefixSet, PrefixSetExpr},
    names::{AsSet, RouteSet},
    primitive::{LiteralPrefixSetEntry, RangeOperator},
};

use crate::query::{PrefixSetPair, Resolver};

macro_rules! debug_eval {
    ( $node:ty ) => {
        log::debug!(concat!("evaluating AST node '", stringify!($node),))
    };
}

pub trait Evaluate {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair>;
}

impl Evaluate for FilterExpr {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        // TODO: implement `fmt::Display` so that we can log the expr here
        log::info!("trying to evaluate filter expression");
        debug_eval!(FilterExpr);
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
        debug_eval!(FilterTerm);
        match self {
            Self::Literal(set_expr, op) => Ok(op.apply_set(set_expr.eval(resolver)?)),
            // TODO
            Self::Named(_) => Err(anyhow!("named filter-sets not yet implemented")),
            Self::Expr(expr) => expr.eval(resolver),
        }
    }
}

trait PrefixSetOp: PrefixOp {
    fn apply_set(&self, pair: PrefixSetPair) -> PrefixSetPair {
        let (ipv4, ipv6) = pair;
        (
            ipv4.map(|set| self.apply_map(set)),
            ipv6.map(|set| self.apply_map(set)),
        )
    }

    fn apply_map<P: IpPrefix>(&self, set: PrefixSet<P>) -> PrefixSet<P> {
        set.ranges()
            .filter_map(|range| {
                self.apply_range(range)
                    .map_err(|err| log::warn!("failed to apply prefix range operator: {}", err))
                    .ok()
            })
            .collect()
    }
}

trait PrefixOp {
    fn apply<P: IpPrefix>(&self, prefix: P) -> Result<IpPrefixRange<P>, prefixset::Error> {
        self.apply_range(prefix.into())
    }

    fn apply_range<P: IpPrefix>(
        &self,
        range: IpPrefixRange<P>,
    ) -> Result<IpPrefixRange<P>, prefixset::Error>;
}

impl PrefixSetOp for RangeOperator {}

impl PrefixOp for RangeOperator {
    fn apply_range<P: IpPrefix>(
        &self,
        range: IpPrefixRange<P>,
    ) -> Result<IpPrefixRange<P>, prefixset::Error> {
        let (lower, upper) = match self {
            Self::None => return Ok(range),
            Self::LessExcl => (*range.range().start() + 1, P::MAX_LENGTH),
            Self::LessIncl => (*range.range().start(), P::MAX_LENGTH),
            Self::Exact(length) => (
                *max(range.range().start(), length),
                *min(range.range().end(), length),
            ),
            Self::Range(upper, lower) => (
                *max(range.range().start(), lower),
                *min(range.range().end(), upper),
            ),
        };
        IpPrefixRange::new(*range.base(), lower, upper)
    }
}

impl Evaluate for PrefixSetExpr {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        debug_eval!(PrefixSetExpr);
        match self {
            Self::Literal(entries) => {
                let sets = entries
                    .into_iter()
                    .filter_map(|entry| {
                        entry
                            .to_prefix_range()
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

trait IntoPrefixRange {
    fn to_prefix_range(
        &self,
    ) -> Result<Either<IpPrefixRange<Ipv4Prefix>, IpPrefixRange<Ipv6Prefix>>>;
}

impl IntoPrefixRange for LiteralPrefixSetEntry {
    fn to_prefix_range(
        &self,
    ) -> Result<Either<IpPrefixRange<Ipv4Prefix>, IpPrefixRange<Ipv6Prefix>>> {
        match self.prefix() {
            IpNet::V4(prefix) => Ok(Either::Left(self.operator().apply((*prefix).into())?)),
            IpNet::V6(prefix) => Ok(Either::Right(self.operator().apply((*prefix).into())?)),
        }
    }
}

impl Evaluate for NamedPrefixSet {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        debug_eval!(NamedPrefixSet);
        match self {
            Self::Any => Ok((Some(PrefixSet::one()), Some(PrefixSet::one()))),
            Self::PeerAs => Err(anyhow!(
                "expected named prefix set, found un-substituted 'PeerAS' token"
            )),
            Self::RouteSet(route_set) => route_set.eval(resolver),
            Self::AsSet(as_set) => as_set.eval(resolver),
            Self::AutNum(autnum) => resolver.job(autnum),
        }
    }
}

impl Evaluate for RouteSet {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        debug_eval!(RouteSet);
        resolver.job(self)
    }
}

impl Evaluate for AsSet {
    fn eval(self, resolver: &mut Resolver) -> Result<PrefixSetPair> {
        debug_eval!(AsSet);
        resolver.job(self)
    }
}
