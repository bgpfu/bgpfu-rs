use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug, Display},
    sync::Arc,
};

use ip::{concrete::PrefixRange, Afi, Ipv4, Ipv6};
use rpsl::expr::MpFilterExpr;

mod compare;

mod eval;
pub(crate) use self::eval::Evaluate;

mod fetch;
pub(crate) use self::fetch::Fetch;

mod load;
pub(crate) use self::load::Load;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Policies<T> {
    map: HashMap<Name, T>,
}

impl<T> Policies<T> {
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }
}

impl Policies<Evaluated> {
    pub(crate) fn succeeded(&self) -> usize {
        self.map
            .values()
            .filter(|item| item.ranges.is_some())
            .count()
    }
}

impl<T> Default for Policies<T> {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Name(Arc<str>);

impl Name {
    fn new<S: AsRef<str>>(name: S) -> Self {
        Self(name.as_ref().into())
    }
}

impl Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for Name {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(PartialEq, Eq)]
pub(crate) struct Candidate {
    filter_expr: MpFilterExpr,
}

impl Debug for Candidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Candidate")
            .field("filter_expr", &self.filter_expr.to_string())
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Installed {
    ipv4: Ranges<Ipv4>,
    ipv6: Ranges<Ipv6>,
}

#[derive(Debug)]
pub(crate) struct Evaluated {
    filter_expr: MpFilterExpr,
    ranges: Option<(Ranges<Ipv4>, Ranges<Ipv6>)>,
}

#[derive(Debug)]
pub(crate) struct Updates<'a> {
    inner: Vec<Update<'a>>,
}

#[derive(Debug)]
pub(crate) enum Update<'a> {
    Delete {
        name: Name,
    },
    Update {
        name: Name,
        filter_expr: &'a MpFilterExpr,
        ipv4: Differences<'a, Ipv4>,
        ipv6: Differences<'a, Ipv6>,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct Ranges<A: Afi> {
    inner: HashSet<PrefixRange<A>>,
}

impl<A: Afi> Ranges<A> {
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    fn iter(&self) -> impl Iterator<Item = &PrefixRange<A>> {
        self.inner.iter()
    }

    fn diff<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = &PrefixRange<A>> {
        self.inner.difference(&other.inner)
    }
}

impl<A: Afi> Default for Ranges<A> {
    fn default() -> Self {
        Self {
            inner: HashSet::new(),
        }
    }
}

impl<A: Afi> FromIterator<PrefixRange<A>> for Ranges<A> {
    fn from_iter<T: IntoIterator<Item = PrefixRange<A>>>(iter: T) -> Self {
        let inner = iter.into_iter().collect();
        Self { inner }
    }
}

#[derive(Debug)]
pub(crate) struct Differences<'a, A: Afi> {
    old: Option<&'a Ranges<A>>,
    new: &'a Ranges<A>,
}
