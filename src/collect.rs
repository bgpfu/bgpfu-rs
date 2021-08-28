use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::panic;
use std::sync::mpsc;
use std::thread;

use ipnet::IpNet;
use irrc::ResponseItem;
use prefixset::{IpPrefix, PrefixSet};

/// Thread-based collection of [`IpPrefix`]s into a [`PrefixSet`].
pub struct Collector<P: IpPrefix>(CollectorSender<P>, Box<CollectorHandle<P>>);

impl<P> Collector<P>
where
    P: 'static + IpPrefix + Send,
    P::Bits: Send,
{
    /// Create a new `Collector` and start the associated thread.
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        Self(tx.into(), rx.into())
    }

    /// Split into a [`CollectorSender`] and a [`CollectorHandle`].
    pub fn split_option(self) -> (Option<CollectorSender<P>>, Option<Box<CollectorHandle<P>>>) {
        (Some(self.0), Some(self.1))
    }
}

/// Send handle for providing [`ResponseItem`]s to the [`Collector`].
pub struct CollectorSender<P>(mpsc::Sender<P>);

impl<P> CollectorSender<P>
where
    P: IpPrefix + TryFrom<IpNet>,
    <P as TryFrom<IpNet>>::Error: fmt::Display,
{
    /// Send an item.
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

/// Thead management handle for [`Collector`] thread.
pub struct CollectorHandle<P: IpPrefix>(thread::JoinHandle<PrefixSet<P>>);

impl<P: IpPrefix> CollectorHandle<P> {
    /// Join collector thread.
    pub fn join(self) -> PrefixSet<P> {
        match self.0.join() {
            Ok(set) => set,
            Err(err) => panic::resume_unwind(err),
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
