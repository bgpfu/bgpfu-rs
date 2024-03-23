use std::collections::HashSet;

use ip::Afi;

use super::{Differences, Evaluated, Installed, Policies, Ranges, Update, Updates};

impl Policies<Evaluated> {
    pub(crate) fn compare<'a>(&'a self, installed: &'a Policies<Installed>) -> Updates<'a> {
        let names = self
            .map
            .keys()
            .chain(installed.map.keys())
            .cloned()
            .collect::<HashSet<_>>();
        let inner = names
            .iter()
            .filter_map(|name| match (self.map.get(name), installed.map.get(name)) {
                (
                    Some(Evaluated {
                        filter_expr,
                        ranges: Some((new_ipv4, new_ipv6)),
                    }),
                    Some(Installed {
                        ipv4: old_ipv4,
                        ipv6: old_ipv6,
                    }),
                ) => Some(Update::Update {
                    name: name.clone(),
                    filter_expr,
                    ipv4: Differences::new(Some(old_ipv4), new_ipv4),
                    ipv6: Differences::new(Some(old_ipv6), new_ipv6),
                }),
                (
                    Some(Evaluated {
                        filter_expr,
                        ranges: Some((new_ipv4, new_ipv6)),
                    }),
                    None,
                ) => Some(Update::Update {
                    name: name.clone(),
                    filter_expr,
                    ipv4: Differences::new(None, new_ipv4),
                    ipv6: Differences::new(None, new_ipv6),
                }),
                (Some(Evaluated { ranges: None, .. }), _) => None,
                (None, Some(_)) => Some(Update::Delete { name: name.clone() }),
                (None, None) => unreachable!(),
            })
            .collect();
        Updates { inner }
    }
}

impl<'a, A: Afi> Differences<'a, A> {
    const fn new(old: Option<&'a Ranges<A>>, new: &'a Ranges<A>) -> Self {
        Self { old, new }
    }
}
