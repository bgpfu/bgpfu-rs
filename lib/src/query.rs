use ip::{Any, Prefix, PrefixSet};

use irrc::{Connection, IrrClient, Query, ResponseItem};

use rpsl::{
    attr::RpslAttribute,
    expr::{
        eval::{Evaluate, Evaluator, Resolver},
        MpFilterExpr,
    },
    names::{AsSet, AutNum, FilterSet, RouteSet},
    obj::{RpslObject, RpslObjectClass},
    primitive::PeerAs,
};

use crate::error::Error;

/// An implementation of [`rpsl::expr::eval::Evaluator`] that resolves RPSL names using the IRRd
/// query protocol.
///
/// # Examples
///
/// ``` no_run
/// use bgpfu::query::RpslEvaluator;
/// use ip::traits::PrefixSet;
/// use rpsl::expr::MpFilterExpr;
///
/// let filter: MpFilterExpr = "AS-FOO AND { 0.0.0.0/0^8-24, ::/0^16-48 }".parse()?;
/// RpslEvaluator::new("whois.radb.net", 43)?
///     .evaluate(filter)?
///     .ranges()
///     .for_each(|range| println!("{range}"));
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct RpslEvaluator {
    conn: Option<Connection>,
}

impl RpslEvaluator {
    /// Construct a new [`Evaluator`].
    ///
    /// # Errors
    ///
    /// An [`Error::Io`] is returned if the connection to the IRRd server cannot be established.
    pub fn new(host: &str, port: u16) -> Result<Self, Error> {
        let addr = format!("{host}:{port}");
        let conn = IrrClient::new(addr).connect()?;
        Ok(Self { conn: Some(conn) })
    }

    fn with_connection<F, T, E>(&mut self, f: F) -> Result<T, Error>
    where
        F: Fn(&mut Self, &mut Connection) -> Result<T, E>,
        E: Into<Error>,
    {
        let mut conn = self
            .conn
            .take()
            .ok_or("failed to take ownership of connection")?;
        let result = f(self, &mut conn).map_err(Into::into);
        self.conn = Some(conn);
        result
    }

    /// Evaluate an RPSL expression.
    ///
    /// This method wraps [`Evaluator::evaluate`], and is provided as a convenience so that the
    /// underlying trait does not have to be brought into scope explicitly.
    ///
    /// # Errors
    ///
    /// See [`Evaluator`] for error handling details.
    pub fn evaluate<'a, T>(
        &mut self,
        expr: T,
    ) -> Result<<Self as Evaluator<'a>>::Output<T>, <Self as Evaluator<'a>>::Error>
    where
        T: Evaluate<'a, Self>,
    {
        <Self as Evaluator>::evaluate(self, expr)
    }
}

impl<'a> Evaluator<'a> for RpslEvaluator {
    type Output<T> = <T as Evaluate<'a, Self>>::Output
    where
        T: Evaluate<'a, Self>;

    type Error = Error;

    fn finalise<T>(&mut self, output: T::Output) -> Result<Self::Output<T>, Self::Error>
    where
        T: Evaluate<'a, Self>,
    {
        Ok(output)
    }

    fn sink_error(&mut self, err: &(dyn std::error::Error + Send + Sync + 'static)) -> bool {
        log::warn!("{err}");
        true
    }
}

impl Resolver<'_, FilterSet, MpFilterExpr> for RpslEvaluator {
    type IError = Error;

    fn resolve(&mut self, filter_set: &FilterSet) -> Result<MpFilterExpr, Self::IError> {
        self.with_connection(|this, conn| {
            conn.pipeline()
                // TODO: this is a bad API - we should be able to determine the required object
                // class from the type of `filter_set`.
                .push(Query::RpslObject(
                    irrc::RpslObjectClass::FilterSet,
                    filter_set.to_string(),
                ))
                .map_err(Error::from)
                .and_then(|pipeline| {
                    pipeline
                        .responses()
                        .find_map(|resp| {
                            this.collect_result(resp.map_err(Error::from).and_then(|item| {
                                if let RpslObject::FilterSet(obj) = item.into_content() {
                                    obj.attrs()
                                        .into_iter()
                                        .find_map(|attr| {
                                            if let RpslAttribute::MpFilter(expr) = attr {
                                                // TODO: shouldn't need to clone here either!
                                                Some(expr.clone())
                                            } else {
                                                None
                                            }
                                        })
                                        .ok_or_else(|| Error::from("no mp-filter attribute found"))
                                } else {
                                    Err(Error::from("unexpected rpsl object"))
                                }
                            }))
                            .transpose()
                        })
                        .unwrap_or_else(|| Ok("NOT ANY".parse()?))
                })
        })
    }
}

impl Resolver<'_, AsSet, PrefixSet<Any>> for RpslEvaluator {
    type IError = Error;

    fn resolve(&mut self, as_set: &AsSet) -> Result<PrefixSet<Any>, Self::IError> {
        self.with_connection(|this, conn| {
            // TODO: shouldn't need to clone here
            conn.pipeline_from_initial(Query::AsSetMembersRecursive(as_set.clone()), |resp| {
                this.collect_result::<_, _, Error>(resp.map(|item| {
                    let autnum = item.into_content();
                    [Query::Ipv4Routes(autnum), Query::Ipv6Routes(autnum)]
                }))
                // TODO: we want a way of providing our own error handling closure
                .unwrap_or_else(|err| {
                    _ = this.sink_error(&err);
                    None
                })
            })
            .and_then(|mut pipeline| {
                this.collect_results(
                    pipeline
                        .responses::<'_, Prefix<Any>>()
                        .map(|resp| resp.map(ResponseItem::into_content)),
                )
            })
        })
    }
}

impl Resolver<'_, RouteSet, PrefixSet<Any>> for RpslEvaluator {
    type IError = Error;

    fn resolve(&mut self, route_set: &RouteSet) -> Result<PrefixSet<Any>, Self::IError> {
        self.with_connection(|this, conn| {
            conn.pipeline()
                // TODO: shouldn't need to clone here
                .push(Query::RouteSetMembersRecursive(route_set.clone()))
                .map_err(Error::from)
                .and_then(|pipeline| {
                    this.collect_results(
                        pipeline
                            .responses::<'_, Prefix<Any>>()
                            .map(|response| response.map(ResponseItem::into_content)),
                    )
                })
        })
    }
}

impl Resolver<'_, AutNum, PrefixSet<Any>> for RpslEvaluator {
    type IError = Error;

    fn resolve(&mut self, autnum: &AutNum) -> Result<PrefixSet<Any>, Self::IError> {
        self.with_connection(|this, conn| {
            conn.pipeline()
                .push(Query::Ipv4Routes(*autnum))?
                .push(Query::Ipv6Routes(*autnum))
                .map_err(Error::from)
                .and_then(|pipeline| {
                    this.collect_results(
                        pipeline
                            .responses::<'_, Prefix<Any>>()
                            .map(|response| response.map(ResponseItem::into_content)),
                    )
                })
        })
    }
}

impl Resolver<'_, PeerAs, PrefixSet<Any>> for RpslEvaluator {
    type IError = Error;

    fn resolve(&mut self, _: &PeerAs) -> Result<PrefixSet<Any>, Self::IError> {
        unimplemented!()
    }
}
