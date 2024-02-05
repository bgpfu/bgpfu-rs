use std::fmt;

use bgpfu::RpslEvaluator;

use ip::any::PrefixSet;

use rpsl::expr::MpFilterExpr;

pub(crate) mod read;
pub(crate) mod write;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PolicyStmt<C: RouteFilterContent> {
    name: String,
    filter_expr: MpFilterExpr,
    content: C,
}

impl<C: RouteFilterContent + fmt::Debug> fmt::Debug for PolicyStmt<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PolicyStmt")
            .field("name", &self.name)
            .field("filter_expr", &self.filter_expr.to_string())
            .field("content", &self.content)
            .finish()
    }
}

pub(crate) trait RouteFilterContent {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Empty;
impl RouteFilterContent for Empty {}

impl RouteFilterContent for PrefixSet {}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct EvaluatedPolicyStmts(Vec<PolicyStmt<PrefixSet>>);

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct CandidatePolicyStmts(Vec<PolicyStmt<Empty>>);

impl CandidatePolicyStmts {
    pub(crate) fn evaluate(self, evaluator: &mut RpslEvaluator) -> EvaluatedPolicyStmts {
        EvaluatedPolicyStmts(
            self.0
                .into_iter()
                .filter_map(|candidate| {
                    match evaluator.evaluate(candidate.filter_expr.clone()) {
                        Ok(set) => Some(set),
                        Err(err) => {
                            log::error!(
                                "failed to evaluate filter expression {}: {err:#}",
                                candidate.filter_expr,
                            );
                            None
                        }
                    }
                    .map(|prefix_set| PolicyStmt {
                        filter_expr: candidate.filter_expr,
                        name: candidate.name,
                        content: prefix_set,
                    })
                })
                .collect(),
        )
    }
}
