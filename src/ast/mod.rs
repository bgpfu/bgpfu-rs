use ipnet::IpNet;
use irrc::types::{AsSet, AutNum, RouteSet};

mod construct;
mod eval;
mod subst;

pub use self::{eval::Evaluate, subst::Substitute};

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
    Named(FilterSetExpr),
    Expr(Box<FilterExpr>),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
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
pub enum FilterSetExpr {
    Pending(Vec<SetNameComp>),
    // TODO: define type for filter-set names.
    Ready(String),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LiteralPrefixSetEntry {
    prefix: IpNet,
    op: PrefixOp,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
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
    RouteSet(RouteSetExpr),
    AsSet(AsSetExpr),
    AutNum(AutNum),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum RouteSetExpr {
    Pending(Vec<SetNameComp>),
    Ready(RouteSet),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum AsSetExpr {
    Pending(Vec<SetNameComp>),
    Ready(AsSet),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum SetNameComp {
    AutNum(AutNum),
    PeerAs,
    Name(String),
}
