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
    pub(crate) fn new(netconf: NetconfOpts, irrd: IrrdOpts, junos: JunosOpts) -> Self {
        Self {
            netconf: Arc::new(netconf),
            irrd: Arc::new(irrd),
            junos: Arc::new(junos),
        }
    }

    pub(crate) fn init_loop(self, frequency: NonZeroU64) -> Loop {
        Loop {
            updater: self,
            period: Duration::from_secs(frequency.into()),
        }
    }

    pub(crate) async fn run(self) -> anyhow::Result<()> {
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

        let config = tokio::task::block_in_place(|| {
            RpslEvaluator::new(self.irrd.host(), self.irrd.port())
                .context("failed to connect to IRRd server")
                .map(|mut evaluator| candidates.evaluate(&mut evaluator))
        })?;

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
            .context("failed to close NETCONF session")
    }
}

pub(crate) struct Loop {
    updater: Updater,
    period: Duration,
}

impl Loop {
    pub(crate) async fn start(self) -> anyhow::Result<()> {
        log::info!("starting updater loop with frequency {:?}", self.period);
        let mut interval = time::interval(self.period);
        let mut sigint =
            signal(SignalKind::interrupt()).context("failed to register handler for SIGINT")?;
        let mut sigterm =
            signal(SignalKind::terminate()).context("failed to register handler for SIGTERM")?;
        loop {
            tokio::select! {
                _ = sigint.recv() => {
                    log::info!("got ctrl-c, exiting");
                    break Ok(())
                }
                _ = sigterm.recv() => {
                    log::info!("got SIGTERM, exiting");
                    break Ok(())
                }
                _ = interval.tick() => {
                    log::info!("starting updater job");
                    let job = self.updater.clone().run();
                    tokio::spawn(job)
                        .await
                        .unwrap_or_else(|err| {
                            log::error!("updater thread panicked: {err}");
                            Ok(())
                        })
                        .unwrap_or_else(|err| {
                            log::error!("updater job failed: {err:#}");
                        });
                    interval.reset();
                }
            }
        }
    }
}
