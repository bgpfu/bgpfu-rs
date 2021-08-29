use std::convert::{TryFrom, TryInto};
use std::str::FromStr;

use anyhow::{anyhow, Error, Result};
use pest::{iterators::Pair, Parser};

use super::{
    AsSetExpr, FilterExpr, FilterSetExpr, FilterTerm, LiteralPrefixSetEntry, NamedPrefixSet,
    PrefixOp, PrefixSetExpr, PrefixSetOp, RouteSetExpr, SetNameComp,
};
use crate::parser::{FilterParser, Rule};

macro_rules! next_into_or {
    ( $pairs:expr => $err:literal ) => {
        $pairs.next().ok_or_else(|| anyhow!($err))?.try_into()?
    };
}

macro_rules! next_parse_or {
    ( $pairs:expr => $err:literal ) => {
        $pairs
            .next()
            .ok_or_else(|| anyhow!($err))?
            .as_str()
            .parse()?
    };
}

macro_rules! debug_construction {
    ( $pair:ident => $node:ty ) => {
        log::debug!(
            concat!(
                "constructing AST node '",
                stringify!($node),
                "' from token pair {:?}: '{}'"
            ),
            $pair.as_rule(),
            $pair.as_str()
        )
    };
}

impl FromStr for FilterExpr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        log::info!("trying to parse filter expression: '{}'", s);
        let root = FilterParser::parse(Rule::filter, s)?
            .next()
            .ok_or_else(|| anyhow!("failed to get root filter expression"))?;
        root.try_into()
    }
}

impl TryFrom<Pair<'_, Rule>> for FilterExpr {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => FilterExpr);
        match pair.as_rule() {
            Rule::filter_expr_unit => Ok(Self::Unit(
                next_into_or!(pair.into_inner() => "failed to get inner filter term"),
            )),
            Rule::filter_expr_not => Ok(Self::Not(
                next_into_or!(pair.into_inner() => "failed to get inner filter term"),
            )),
            Rule::filter_expr_and => {
                let mut pairs = pair.into_inner();
                let (left_term, right_term) = (
                    next_into_or!(pairs => "failed to get left inner filter term"),
                    next_into_or!(pairs => "failed to get right inner filter term"),
                );
                Ok(Self::And(left_term, right_term))
            }
            Rule::filter_expr_or => {
                let mut pairs = pair.into_inner();
                let (left_term, right_term) = (
                    next_into_or!(pairs => "failed to get left inner filter term"),
                    next_into_or!(pairs => "failed to get right inner filter term"),
                );
                Ok(Self::Or(left_term, right_term))
            }
            _ => Err(anyhow!(
                "expected a filter expression, got {:?}: '{}'",
                pair.as_rule(),
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for FilterTerm {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => FilterTerm);
        match pair.as_rule() {
            Rule::literal_filter => {
                let mut pairs = pair.into_inner();
                Ok(Self::Literal(
                    next_into_or!(pairs => "failed to get inner prefix set expression"),
                    match pairs.next() {
                        Some(inner) => inner.try_into()?,
                        None => PrefixSetOp::None,
                    },
                ))
            }
            Rule::named_filter => Ok(Self::Named(
                next_into_or!(pair.into_inner() => "failed to get inner filter-set name"),
            )),
            Rule::filter_expr_unit
            | Rule::filter_expr_not
            | Rule::filter_expr_and
            | Rule::filter_expr_or => Ok(Self::Expr(Box::new(pair.try_into()?))),
            _ => Err(anyhow!("expected filter term, got {}", pair.as_str())),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for PrefixSetOp {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => PrefixSetOp);
        match pair.as_rule() {
            Rule::less_excl => Ok(Self::LessExcl),
            Rule::less_incl => Ok(Self::LessIncl),
            _ => Err(anyhow!(
                "expected a prefix set operation, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for PrefixSetExpr {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => PrefixSetExpr);
        match pair.as_rule() {
            Rule::literal_prefix_set => Ok(Self::Literal(
                pair.into_inner()
                    .map(|inner| inner.try_into())
                    .collect::<Result<_>>()?,
            )),
            Rule::named_prefix_set => Ok(Self::Named(
                next_into_or!(pair.into_inner() => "failed to prefix set name"),
            )),
            _ => Err(anyhow!(
                "expected prefix set expression, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for FilterSetExpr {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => FilterSetExpr);
        match pair.as_rule() {
            Rule::filter_set => Ok(Self::Pending(
                pair.into_inner()
                    .map(|inner| inner.try_into())
                    .collect::<Result<_>>()?,
            )),
            _ => Err(anyhow!(
                "expected filter-set expression, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for LiteralPrefixSetEntry {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => LiteralPrefixSetEntry);
        let mut pairs = pair.into_inner();
        let prefix = next_parse_or!(pairs => "failed to get inner prefix");
        let op = match pairs.next() {
            Some(inner) => inner.try_into()?,
            None => PrefixOp::None,
        };
        Ok(Self { prefix, op })
    }
}

impl TryFrom<Pair<'_, Rule>> for PrefixOp {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => PrefixOp);
        match pair.as_rule() {
            Rule::less_excl => Ok(Self::LessExcl),
            Rule::less_incl => Ok(Self::LessIncl),
            Rule::exact => Ok(Self::Exact(
                next_parse_or!(pair.into_inner() => "failed to get operand for range operation"),
            )),
            Rule::range => {
                let mut pairs = pair.into_inner();
                Ok(Self::Range(
                    next_parse_or!(pairs => "failed to get lower operand for range operation"),
                    next_parse_or!(pairs => "failed to get upper operand for range operation"),
                ))
            }
            _ => Err(anyhow!(
                "expected a prefix range operation, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for NamedPrefixSet {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => NamedPrefixSet);
        match pair.as_rule() {
            Rule::any_route => Ok(Self::Any),
            Rule::peeras => Ok(Self::PeerAs),
            Rule::route_set => Ok(Self::RouteSet(pair.try_into()?)),
            Rule::as_set => Ok(Self::AsSet(pair.try_into()?)),
            Rule::autnum => Ok(Self::AutNum(pair.as_str().parse()?)),
            _ => Err(anyhow!(
                "expected a named prefix variant, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for RouteSetExpr {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => RouteSetExpr);
        match pair.as_rule() {
            Rule::route_set => Ok(Self::Pending(
                pair.into_inner()
                    .map(|inner| inner.try_into())
                    .collect::<Result<_>>()?,
            )),
            _ => Err(anyhow!(
                "expected route-set expression, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for AsSetExpr {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => AsSetExpr);
        match pair.as_rule() {
            Rule::as_set => Ok(Self::Pending(
                pair.into_inner()
                    .map(|inner| inner.try_into())
                    .collect::<Result<_>>()?,
            )),
            _ => Err(anyhow!(
                "expected as-set expression, got '{}'",
                pair.as_str()
            )),
        }
    }
}

impl TryFrom<Pair<'_, Rule>> for SetNameComp {
    type Error = Error;

    fn try_from(pair: Pair<Rule>) -> Result<Self> {
        debug_construction!(pair => SetNameComp);
        match pair.as_rule() {
            Rule::autnum => Ok(Self::AutNum(pair.as_str().parse()?)),
            Rule::peeras => Ok(Self::PeerAs),
            Rule::filter_set_name | Rule::route_set_name | Rule::as_set_name => {
                Ok(Self::Name(pair.as_str().to_string()))
            }
            _ => Err(anyhow!(
                "expected set name component, got '{}'",
                pair.as_str()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use ipnet::IpNet;
    use paste::paste;

    use super::*;

    macro_rules! test_exprs {
        ( $( $name:ident: $query:literal => $expr:expr ),* $(,)? ) => {
            paste! {
                $(
                    #[test]
                    fn [< $name _expr>]() {
                        let ast: FilterExpr = dbg!($query.parse().unwrap());
                        assert_eq!(ast, $expr)
                    }
                )*
            }
        }
    }

    test_exprs! {
        single_autnum: "AS65000" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::AutNum("AS65000".parse().unwrap())),
                PrefixSetOp::None
            )),
        simple_as_set: "AS-FOO" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::AsSet(AsSetExpr::Pending(vec![
                    SetNameComp::Name("AS-FOO".to_string())
                ]))),
                PrefixSetOp::None
            )),
        hierarchical_as_set: "AS65000:AS-FOO" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::AsSet(AsSetExpr::Pending(vec![
                    SetNameComp::AutNum("AS65000".parse().unwrap()),
                    SetNameComp::Name("AS-FOO".to_string())
                ]))),
                PrefixSetOp::None
            )),
        simple_route_set: "RS-FOO" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::RouteSet(RouteSetExpr::Pending(vec![
                    SetNameComp::Name("RS-FOO".to_string())
                ]))),
                PrefixSetOp::None
            )),
        hierarchical_route_set: "AS65000:RS-FOO" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::RouteSet(RouteSetExpr::Pending(vec![
                    SetNameComp::AutNum("AS65000".parse().unwrap()),
                    SetNameComp::Name("RS-FOO".to_string())
                ]))),
                PrefixSetOp::None
            )),
        peeras: "PeerAS" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::PeerAs),
                PrefixSetOp::None
            )),
        any: "ANY" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Named(NamedPrefixSet::Any),
                PrefixSetOp::None
            )),
        empty_literal_prefix_set: "{}" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Literal(vec![]),
                PrefixSetOp::None,
            )),
        single_literal_prefix_set: "{ 192.0.2.0/24^- }" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Literal(vec![
                    LiteralPrefixSetEntry {
                        prefix: IpNet::V4("192.0.2.0/24".parse().unwrap()),
                        op: PrefixOp::LessExcl,
                    },
                ]),
                PrefixSetOp::None,
            )),
        multi_literal_prefix_set: "{ 192.0.2.0/25^+, 192.0.2.128/26^27, 2001:db8::/32^48-56 }" =>
            FilterExpr::Unit(FilterTerm::Literal(
                PrefixSetExpr::Literal(vec![
                    LiteralPrefixSetEntry {
                        prefix: IpNet::V4("192.0.2.0/25".parse().unwrap()),
                        op: PrefixOp::LessIncl,
                    },
                    LiteralPrefixSetEntry {
                        prefix: IpNet::V4("192.0.2.128/26".parse().unwrap()),
                        op: PrefixOp::Exact(27),
                    },
                    LiteralPrefixSetEntry {
                        prefix: IpNet::V6("2001:db8::/32".parse().unwrap()),
                        op: PrefixOp::Range(48, 56),
                    },
                ]),
                PrefixSetOp::None,
            )),

        // Parenthesised
        parens_single_autnum: "(AS65000)" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Named(NamedPrefixSet::AutNum("AS65000".parse().unwrap())),
                    PrefixSetOp::None
                ))
            ))),
        parens_hierarchical_as_set: "(AS65000:AS-FOO:PeerAS)" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Named(NamedPrefixSet::AsSet(AsSetExpr::Pending(vec![
                        SetNameComp::AutNum("AS65000".parse().unwrap()),
                        SetNameComp::Name("AS-FOO".to_string()),
                        SetNameComp::PeerAs,
                    ]))),
                    PrefixSetOp::None
                ))
            ))),
        parens_peeras: "(PeerAS)" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Named(NamedPrefixSet::PeerAs),
                    PrefixSetOp::None
                ))
            ))),
        parens_any: "(ANY)" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Named(NamedPrefixSet::Any),
                    PrefixSetOp::None
                ))
            ))),
        parens_empty_literal_prefix_set: "({})" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Literal(vec![]),
                    PrefixSetOp::None,
                ))
            ))),
        parens_single_literal_prefix_set: "({ 192.0.2.0/24^- })" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Literal(vec![
                        LiteralPrefixSetEntry {
                            prefix: IpNet::V4("192.0.2.0/24".parse().unwrap()),
                            op: PrefixOp::LessExcl,
                        },
                    ]),
                    PrefixSetOp::None,
                ))
            ))),
        parens_multi_literal_prefix_set: "({ 192.0.2.0/25^+, 192.0.2.128/26^27, 2001:db8::/32^48-56 })" =>
            FilterExpr::Unit(FilterTerm::Expr(Box::new(
                FilterExpr::Unit(FilterTerm::Literal(
                    PrefixSetExpr::Literal(vec![
                        LiteralPrefixSetEntry {
                            prefix: IpNet::V4("192.0.2.0/25".parse().unwrap()),
                            op: PrefixOp::LessIncl,
                        },
                        LiteralPrefixSetEntry {
                            prefix: IpNet::V4("192.0.2.128/26".parse().unwrap()),
                            op: PrefixOp::Exact(27),
                        },
                        LiteralPrefixSetEntry {
                            prefix: IpNet::V6("2001:db8::/32".parse().unwrap()),
                            op: PrefixOp::Range(48, 56),
                        },
                    ]),
                    PrefixSetOp::None,
                ))
            ))),
    }
}
