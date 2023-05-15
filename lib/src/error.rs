use rpsl::{error::ParseError, expr::eval::EvaluationError};

/// Error condition variants.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// IRR query protocol errors.
    #[error(transparent)]
    Irr(#[from] irrc::Error),
    /// RPSL expression evaluation errors.
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
    /// RPSL parsing errors.
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("{0}")]
    General(String),
}

impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::General(value.to_string())
    }
}
