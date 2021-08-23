use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::sync::mpsc;
use std::thread;

use ipnet::IpNet;
use irrc::ResponseItem;
use prefixset::{IpPrefix, PrefixSet};

pub struct Collector<P: IpPrefix>(pub CollectorSender<P>, pub Box<CollectorHandle<P>>);

impl<P> Collector<P>
where
    P: 'static + IpPrefix + Send,
    P::Bits: Send,
{
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        Self(tx.into(), rx.into())
    }
}

pub struct CollectorSender<P>(mpsc::Sender<P>);

impl<P> CollectorSender<P>
where
    P: IpPrefix + TryFrom<IpNet>,
    <P as TryFrom<IpNet>>::Error: fmt::Display,
{
    pub fn collect(&self, item: ResponseItem<IpNet>) {
        match item.into_content().try_into() {
            Ok(prefix) => {
                if let Err(err) = self.0.send(prefix) {
                    log::warn!("failed to send prefix to collector: {}", err);
                }
            }
            Err(err) => log::warn!("failed to parse prefix: {}", err),
        }
    }
}

impl<P> From<mpsc::Sender<P>> for CollectorSender<P> {
    fn from(tx: mpsc::Sender<P>) -> Self {
        Self(tx)
    }
}

pub struct CollectorHandle<P: IpPrefix>(thread::JoinHandle<PrefixSet<P>>);

impl<P: IpPrefix> CollectorHandle<P> {
    pub fn print(self) {
        match self.0.join() {
            Ok(set) => set.ranges().for_each(|range| println!("{}", range)),
            Err(err) => log::error!("failed to join set builder thread: {:?}", err),
        }
    }
}

impl<P> From<mpsc::Receiver<P>> for Box<CollectorHandle<P>>
where
    P: 'static + IpPrefix + Send,
    P::Bits: Send,
{
    fn from(rx: mpsc::Receiver<P>) -> Self {
        Box::new(CollectorHandle(thread::spawn(move || {
            rx.iter()
                .inspect(|prefix| log::debug!("adding prefix {} to prefix set", prefix))
                .collect::<PrefixSet<_>>()
        })))
    }
}
