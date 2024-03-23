use std::{num::NonZeroU64, sync::Arc};

use anyhow::Context;

use bgpfu::RpslEvaluator;

use tokio::{
    signal::unix::{signal, SignalKind},
    task::JoinHandle,
    time::{self, Duration},
};

use crate::{
    cli::{IrrdOpts, JunosOpts, NetconfOpts},
    netconf::Client,
    policies::{Candidate, Evaluate, Installed, Policies},
};

#[derive(Debug, Clone)]
pub(crate) struct Updater {
    netconf: Arc<NetconfOpts>,
    irrd: Arc<IrrdOpts>,
    junos: Arc<JunosOpts>,
}

impl Updater {
    #[tracing::instrument(level = "debug")]
    pub(crate) fn new(netconf: NetconfOpts, irrd: IrrdOpts, junos: JunosOpts) -> Self {
        Self {
            netconf: Arc::new(netconf),
            irrd: Arc::new(irrd),
            junos: Arc::new(junos),
        }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) fn init_loop(self, frequency: NonZeroU64) -> Loop {
        Loop {
            updater: self,
            period: Duration::from_secs(frequency.into()),
        }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub(crate) async fn run(self) -> anyhow::Result<()> {
        tracing::info!("starting update");

        let mut netconf_client = Client::connect(
            self.netconf.host(),
            self.netconf.port(),
            self.netconf.ca_cert_path(),
            self.netconf.client_cert_path(),
            self.netconf.client_key_path(),
            self.netconf.tls_server_name(),
        )
        .await
        .context("failed to establish NETCONF session")?
        .open_db(self.junos.ephemeral_db())
        .await
        .context("failed to open ephemeral database")?;

        let evaluate_candidates = netconf_client
            .fetch_config::<Policies<Candidate>>()
            .await
            .context("failed to request candidate configuration")
            .map(|response| {
                tokio::spawn(async move {
                    let policies = response
                        .await
                        .context("failed to fetch candidate policy statements")?;
                    tracing::info!(
                        "successfully fetched {} candidate policy statements",
                        policies.len()
                    );
                    let evaluated = tokio::task::block_in_place(|| {
                        RpslEvaluator::new(self.irrd.host(), self.irrd.port())
                            .context("failed to connect to IRRd server")
                            .map(|mut evaluator| policies.evaluate(&mut evaluator))
                    })?;
                    tracing::info!(
                        "successfully evaluated {} of {} policy statements",
                        evaluated.succeeded(),
                        evaluated.len()
                    );
                    Ok(evaluated)
                })
            })?;

        let fetch_installed = netconf_client
            .fetch_config::<Policies<Installed>>()
            .await
            .context("failed to request installed ephemeral configuration")
            .map(|response| {
                tokio::spawn(async move {
                    let policies = response
                        .await
                        .context("failed to fetch installed policy statements")?;
                    tracing::info!(
                        "successfully fetched {} installed policy statements",
                        policies.len()
                    );
                    Ok(policies)
                })
            })?;

        let (evaluated, installed) = tokio::try_join!(
            handle_task(evaluate_candidates),
            handle_task(fetch_installed)
        )?;

        let updates = evaluated.compare(&installed);

        netconf_client
            .load_config(updates)
            .await
            .context("failed to load configuration")?
            .commit_config()
            .await
            .context("failed to commit to ephemeral database")?;

        netconf_client
            .close_db()
            .await
            .context("failed to close ephemeral database")?
            .close()
            .await
            .context("failed to close NETCONF session")?;

        tracing::info!("policies successfully updated");
        Ok(())
    }
}

pub(crate) struct Loop {
    updater: Updater,
    period: Duration,
}

const MIN_BACKOFF: Duration = Duration::from_secs(60);

impl Loop {
    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) async fn start(self) -> anyhow::Result<()> {
        tracing::info!("starting updater loop with frequency {:?}", self.period);
        let mut interval = time::interval(self.period);
        let mut backoff = MIN_BACKOFF;
        let mut sigint =
            signal(SignalKind::interrupt()).context("failed to register handler for SIGINT")?;
        let mut sigterm =
            signal(SignalKind::terminate()).context("failed to register handler for SIGTERM")?;
        let mut sighup =
            signal(SignalKind::hangup()).context("failed to register handler for SIGHUP")?;
        loop {
            tokio::select! {
                _ = sigint.recv() => {
                    tracing::info!("got ctrl-c, exiting");
                    break Ok(())
                }
                _ = sigterm.recv() => {
                    tracing::info!("got SIGTERM, exiting");
                    break Ok(())
                }
                _ = sighup.recv() => {
                    tracing::info!("got SIGHUP, resetting interval timer");
                    interval.reset_immediately();
                }
                _ = interval.tick() => {
                    tracing::info!("starting updater job");
                    let job = self.updater.clone().run();
                    match handle_task(tokio::spawn(job)).await {
                        Ok(()) => {
                            interval.reset();
                            backoff = MIN_BACKOFF;
                        }
                        Err(err) => {
                            tracing::error!("updater job failed: {err:#}");
                            interval.reset_after(backoff);
                            backoff *= 2;
                        }
                    }
                }
            }
        }
    }
}

async fn handle_task<T: Send>(handle: JoinHandle<anyhow::Result<T>>) -> anyhow::Result<T> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err).context("task failed"),
        Err(err) => Err(err).context("task panicked"),
    }
}
