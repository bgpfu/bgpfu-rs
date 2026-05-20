use std::path::Path;

use anyhow::Context;

use rustls_pki_types::pem::PemObject;

use tokio::{fs::File, io::AsyncReadExt};

pub(super) async fn read_item<T>(path: &Path) -> anyhow::Result<T>
where
    T: PemObject,
{
    let input = {
        let mut buf = Vec::new();
        _ = File::open(path)
            .await
            .context("failed to open PEM file")?
            .read_to_end(&mut buf)
            .await
            .context("failed to read PEM file contents")?;
        buf
    };
    T::from_pem_slice(&input).context("error while decoding PEM object")
}
