use bgpfu::RpslEvaluator;
use ip::traits::PrefixSet;

use super::{Candidate, Evaluated, Policies};

pub(crate) trait Evaluate {
    type Evaluated;

    fn evaluate(self, evaluator: &mut RpslEvaluator) -> Self::Evaluated;
}

impl Evaluate for Policies<Candidate> {
    type Evaluated = Policies<Evaluated>;

    #[tracing::instrument(skip(evaluator), level = "trace")]
    fn evaluate(self, evaluator: &mut RpslEvaluator) -> Policies<Evaluated> {
        tracing::debug!("trying to evaluate {} candidate policies", self.map.len());
        let map = self
            .map
            .into_iter()
            .map(|(name, candidate)| {
                let evaluated = candidate.evaluate(evaluator);
                (name, evaluated)
            })
            .collect();
        Policies { map }
    }
}

impl Evaluate for Candidate {
    type Evaluated = Evaluated;

    #[tracing::instrument(skip(self, evaluator), level = "debug")]
    fn evaluate(self, evaluator: &mut RpslEvaluator) -> Evaluated {
        tracing::debug!(
            %self.filter_expr,
            "trying to evaluate filter expression"
        );
        let ranges = evaluator
            .evaluate(self.filter_expr.clone())
            .map_err(|err| {
                tracing::error!(
                    "failed to evaluate filter expression {}: {err:#}",
                    self.filter_expr,
                );
            })
            .map(|set| {
                let (ipv4, ipv6) = set.as_partitions();
                (ipv4.ranges().collect(), ipv6.ranges().collect())
            })
            .ok();
        Evaluated {
            filter_expr: self.filter_expr,
            ranges,
        }
    }
}
