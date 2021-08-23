use std::env::args;
use std::panic;
use std::thread;

use bgpfu::Collector;
use irrc::{IrrClient, Query, QueryResult};
use prefixset::{Ipv4Prefix, Ipv6Prefix};
use simple_logger::SimpleLogger;

fn main() -> QueryResult<()> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .init()
        .unwrap();
    let args: Vec<String> = args().collect();
    let host = format!("{}:43", args[1]);
    let object = args[2].parse().unwrap();
    let (Collector(tx_ipv4, jh_ipv4), Collector(tx_ipv6, jh_ipv6)) = (
        Collector::<Ipv4Prefix>::spawn(),
        Collector::<Ipv6Prefix>::spawn(),
    );
    let query_thread = thread::spawn(move || -> QueryResult<()> {
        IrrClient::new(host)
            .connect()?
            .pipeline_from_initial(Query::AsSetMembersRecursive(object), |item| {
                item.map(|item| {
                    [
                        Query::Ipv4Routes(*item.content()),
                        Query::Ipv6Routes(*item.content()),
                    ]
                })
                .map_err(|err| log::warn!("failed to parse aut-num: {}", err))
                .ok()
            })?
            .responses()
            .filter_map(|item| {
                item.map_err(|err| log::warn!("failed to parse prefix: {}", err))
                    .ok()
            })
            .for_each(|item| match item.query() {
                Query::Ipv4Routes(_) => tx_ipv4.collect(item),
                Query::Ipv6Routes(_) => tx_ipv6.collect(item),
                _ => unreachable!(),
            });
        Ok(())
    });
    match query_thread.join() {
        Ok(result) => result?,
        Err(err) => panic::resume_unwind(err),
    };
    jh_ipv4.print();
    jh_ipv6.print();
    Ok(())
}
