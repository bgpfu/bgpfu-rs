use rpsl::{attr::AttributeType, error::ParseError, expr::eval::EvaluationError, obj::RpslObject};

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
    /// The connection to the IRRd server couldn't be acquired.
    #[error("failed to acquire the connection to the IRRd server")]
    AcquireConnection,
    /// The required attribute was not found in the RPSL object.
    #[error("no {0} attribute found in RPSL object {1}")]
    FindAttribute(AttributeType, RpslObject),
    /// An unexpected RPSL object type was received.
    #[error("unexpected RPSL object {0}")]
    RpslObjectClass(RpslObject),
}
