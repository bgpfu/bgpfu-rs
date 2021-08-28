use ipnet::IpNet;
use irrc::types::{AsSet, AutNum, RouteSet};

mod construct;
mod eval;

pub use eval::Evaluate;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FilterExpr {
    Unit(FilterTerm),
    Not(FilterTerm),
    And(FilterTerm, FilterTerm),
    Or(FilterTerm, FilterTerm),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FilterTerm {
    Literal(PrefixSetExpr, PrefixSetOp),
    Named(String),
    Expr(Box<FilterExpr>),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum PrefixSetOp {
    None,
    LessExcl,
    LessIncl,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum PrefixSetExpr {
    Literal(Vec<LiteralPrefixSetEntry>),
    Named(NamedPrefixSet),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LiteralPrefixSetEntry {
    prefix: IpNet,
    op: PrefixOp,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
enum PrefixOp {
    None,
    LessExcl,
    LessIncl,
    Exact(u8),
    Range(u8, u8),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum NamedPrefixSet {
    Any,
    PeerAs,
    RouteSet(RouteSet),
    AsSet(AsSet),
    AutNum(AutNum),
}
