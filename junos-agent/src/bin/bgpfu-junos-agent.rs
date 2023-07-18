#[tokio::main]
async fn main() -> anyhow::Result<()> {
    bgpfu_junos_agent::main().await.map_err(|err| {
        log::error!("{err}");
        err
    })
}
