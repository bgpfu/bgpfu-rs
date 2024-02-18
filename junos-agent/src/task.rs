use std::{num::NonZeroU64, sync::Arc};

use anyhow::Context;

use bgpfu::RpslEvaluator;

use tokio::{
    signal::unix::{signal, SignalKind},
    time::{self, Duration},
};

use crate::{
    cli::{IrrdOpts, JunosOpts, NetconfOpts},
    config::CandidatePolicyStmts,
    netconf::Client,
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

        let candidates = netconf_client
            .get_config::<CandidatePolicyStmts>()
            .await
            .context("failed to get candidate policies")?;

        tracing::info!("found {} candidate policy statements", candidates.len());

        let config = tokio::task::block_in_place(|| {
            RpslEvaluator::new(self.irrd.host(), self.irrd.port())
                .context("failed to connect to IRRd server")
                .map(|mut evaluator| candidates.evaluate(&mut evaluator))
        })?;

        tracing::info!("successfully evaluated {} policy statements", config.len());

        netconf_client
            .load_config(config)
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

impl Loop {
    #[tracing::instrument(skip(self), level = "trace")]
    pub(crate) async fn start(self) -> anyhow::Result<()> {
        tracing::info!("starting updater loop with frequency {:?}", self.period);
        let mut interval = time::interval(self.period);
        let mut sigint =
            signal(SignalKind::interrupt()).context("failed to register handler for SIGINT")?;
        let mut sigterm =
            signal(SignalKind::terminate()).context("failed to register handler for SIGTERM")?;
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
                _ = interval.tick() => {
                    tracing::info!("starting updater job");
                    let job = self.updater.clone().run();
                    tokio::spawn(job)
                        .await
                        .unwrap_or_else(|err| {
                            tracing::error!("updater thread panicked: {err}");
                            Ok(())
                        })
                        .unwrap_or_else(|err| {
                            tracing::error!("updater job failed: {err:#}");
                        });
                    interval.reset();
                }
            }
        }
    }
}
