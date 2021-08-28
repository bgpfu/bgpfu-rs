use std::mem;

use anyhow::Result;
use irrc::{
    types::{AsSet, AutNum, RouteSet},
    Connection, IrrClient, Pipeline, Query, QueryResult,
};
use prefixset::{Ipv4Prefix, Ipv6Prefix, PrefixSet};
use strum::{EnumString, EnumVariantNames};

use crate::{
    ast::{Evaluate, FilterExpr},
    cli::Args,
    collect::{Collector, CollectorHandle},
};

/// Alias for the return type of [`Resolver::resolve`].
pub type PrefixSetPair = (Option<PrefixSet<Ipv4Prefix>>, Option<PrefixSet<Ipv6Prefix>>);

/// An object that can be transformed into a [`Pipeline`].
pub trait IntoPipeline {
    /// Transform into a [`Pipeline`] using a [`Resolver`].
    fn into_pipeline<'a>(self, resolver: &'a mut Resolver) -> QueryResult<Pipeline<'a>>;
}

impl IntoPipeline for AutNum {
    fn into_pipeline<'a>(self, resolver: &'a mut Resolver) -> QueryResult<Pipeline<'a>> {
        let queries = resolver.af_filter().queries(self);
        let mut pipeline = resolver.conn_mut().pipeline();
        pipeline.extend(queries);
        Ok(pipeline)
    }
}

impl IntoPipeline for AsSet {
    fn into_pipeline<'a>(self, resolver: &'a mut Resolver) -> QueryResult<Pipeline<'a>> {
        let af_filter = resolver.af_filter().to_owned();
        resolver
            .conn_mut()
            .pipeline_from_initial(Query::AsSetMembersRecursive(self), |result| {
                result
                    .map(|item| af_filter.queries(*item.content()))
                    .map_err(|err| log::warn!("failed to parse aut-num: {}", err))
                    .ok()
            })
    }
}

impl IntoPipeline for RouteSet {
    fn into_pipeline<'a>(self, resolver: &'a mut Resolver) -> QueryResult<Pipeline<'a>> {
        let mut pipeline = resolver.conn_mut().pipeline();
        pipeline.push(Query::RouteSetMembersRecursive(self))?;
        Ok(pipeline)
    }
}

/// Query filter by address family.
#[derive(Copy, Clone, Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum AddressFamilyFilter {
    /// Query both IPv4 and IPv6 prefixes.
    Any,
    /// Query only IPv4 prefixes.
    Ipv4,
    /// Query only IPv6 prefixes.
    Ipv6,
}

impl AddressFamilyFilter {
    /// Are IPv4 prefixes included in the filter?
    pub fn with_ipv4(&self) -> bool {
        matches!(self, Self::Ipv4 | Self::Any)
    }

    /// Are IPv6 prefixes included in the filter?
    pub fn with_ipv6(&self) -> bool {
        matches!(self, Self::Ipv6 | Self::Any)
    }

    /// Construct IRR queries for searching for `route(6)` objects by `origin`.
    pub fn queries(&self, autnum: AutNum) -> impl Iterator<Item = Query> {
        vec![
            if self.with_ipv4() {
                Some(Query::Ipv4Routes(autnum))
            } else {
                None
            },
            if self.with_ipv6() {
                Some(Query::Ipv6Routes(autnum))
            } else {
                None
            },
        ]
        .into_iter()
        .flatten()
    }
}

/// Thread based prefix set query resolver.
pub struct Resolver<'a> {
    conn: Connection,
    af_filter: &'a AddressFamilyFilter,
}

impl<'a> Resolver<'a> {
    /// Construct new resolver
    pub fn new(args: &'a Args) -> Result<Self> {
        let conn = IrrClient::new(args.addr()).connect()?;
        let af_filter = args.address_family();
        Ok(Self { conn, af_filter })
    }

    /// Get a mutable ref to the underlying [`Connection`].
    fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    /// Get a ref to the [`AddressFamilyFilter`].
    fn af_filter(&self) -> &AddressFamilyFilter {
        self.af_filter
    }

    /// Filter a pair of [`PrefixSet`]s.
    pub fn filter_pair(
        &self,
        sets: (PrefixSet<Ipv4Prefix>, PrefixSet<Ipv6Prefix>),
    ) -> PrefixSetPair {
        let (ipv4, ipv6) = sets;
        (
            if self.af_filter().with_ipv4() {
                Some(ipv4)
            } else {
                None
            },
            if self.af_filter().with_ipv6() {
                Some(ipv6)
            } else {
                None
            },
        )
    }

    /// Resolve a [`FilterExpr`] into a [`PrefixSet`].
    pub fn resolve(&mut self, filter: FilterExpr) -> Result<PrefixSetPair> {
        filter.eval(self)
    }

    /// Spawn a resolution [`Job`].
    pub fn job<T>(&mut self, object: T) -> Result<PrefixSetPair>
    where
        T: IntoPipeline,
    {
        Job::spawn(self, object)?.join()
    }
}

/// A query resolution job using a [`Resolver`].
pub struct Job<'a, 'b>
where
    'a: 'b,
{
    #[allow(dead_code)]
    resolver: &'b mut Resolver<'a>,
    ipv4_collector_handle: Option<Box<CollectorHandle<Ipv4Prefix>>>,
    ipv6_collector_handle: Option<Box<CollectorHandle<Ipv6Prefix>>>,
}

impl<'a, 'b> Job<'a, 'b>
where
    'a: 'b,
{
    /// Spawn query and collection threads.
    pub fn spawn<T>(resolver: &'b mut Resolver<'a>, object: T) -> Result<Self>
    where
        T: IntoPipeline,
    {
        let (ipv4_collector_tx, ipv4_collector_handle) = if resolver.af_filter().with_ipv4() {
            Collector::<Ipv4Prefix>::spawn().split_option()
        } else {
            (None, None)
        };
        let (ipv6_collector_tx, ipv6_collector_handle) = if resolver.af_filter().with_ipv6() {
            Collector::<Ipv6Prefix>::spawn().split_option()
        } else {
            (None, None)
        };
        object
            .into_pipeline(resolver)?
            .responses()
            .filter_map(|item| {
                item.map_err(|err| log::warn!("failed to parse prefix: {}", err))
                    .ok()
            })
            .for_each(|item| match item.query() {
                Query::Ipv4Routes(_) | Query::RouteSetMembersRecursive(_) => {
                    if let Some(ref tx) = ipv4_collector_tx {
                        tx.collect(item);
                    } else {
                        log::error!("unexpected IPv4 response item received: {:?}", item);
                    }
                }
                Query::Ipv6Routes(_) => {
                    if let Some(ref tx) = ipv6_collector_tx {
                        tx.collect(item);
                    } else {
                        log::error!("unexpected IPv6 response item received: {:?}", item);
                    }
                }
                _ => unreachable!(),
            });
        mem::drop(ipv4_collector_tx);
        mem::drop(ipv6_collector_tx);
        Ok(Self {
            resolver,
            ipv4_collector_handle,
            ipv6_collector_handle,
        })
    }

    /// Join all threads, and return results.
    pub fn join(self) -> Result<PrefixSetPair> {
        Ok((
            self.ipv4_collector_handle.map(|handle| handle.join()),
            self.ipv6_collector_handle.map(|handle| handle.join()),
        ))
    }
}
