use std::num::NonZeroU64;

use anyhow::Context;

use bgpfu::RpslEvaluator;

use tokio::{
    signal::unix::{signal, SignalKind},
    time::{self, Duration},
};

use crate::{config::CandidatePolicyStmts, jet::Transport};

#[derive(Debug, Clone)]
pub(crate) struct Updater {
    jet_transport: Transport,
    jet_username: String,
    jet_password: String,
    ephemeral_db_instance: String,
    irrd_host: String,
    irrd_port: u16,
}

impl Updater {
    pub(crate) const fn new(
        jet_transport: Transport,
        jet_username: String,
        jet_password: String,
        ephemeral_db_instance: String,
        irrd_host: String,
        irrd_port: u16,
    ) -> Self {
        Self {
            jet_transport,
            jet_username,
            jet_password,
            ephemeral_db_instance,
            irrd_host,
            irrd_port,
        }
    }

    pub(crate) fn init_loop(self, frequency: NonZeroU64) -> Loop {
        Loop {
            updater: self,
            period: Duration::from_secs(frequency.into()),
        }
    }

    pub(crate) async fn run(self) -> anyhow::Result<()> {
        let mut jet_client = self
            .jet_transport
            .connect()
            .await?
            .authenticate(self.jet_username, self.jet_password)
            .await?;

        let candidates = jet_client
            .get_running_config::<CandidatePolicyStmts>()
            .await?;

        let config = tokio::task::block_in_place(|| {
            RpslEvaluator::new(&self.irrd_host, self.irrd_port)
                .context("failed to connect to IRRd server")
                .map(|mut evaluator| candidates.evaluate(&mut evaluator))
        })?;

        jet_client
            .clear_ephemeral_config(
                self.ephemeral_db_instance.clone(),
                "policy-options".to_string(),
            )
            .await
            .unwrap_or_else(|err| log::warn!("failed to clear ephemeral configuration: {err}"));

        jet_client
            .set_ephemeral_config(self.ephemeral_db_instance, config)
            .await?;

        drop(jet_client);

        Ok(())
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
                            log::error!("updater job failed: {err}");
                        });
                    interval.reset();
                }
            }
        }
    }
}
