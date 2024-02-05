use std::path::Path;

use anyhow::Context;

use rustls_pemfile::{read_one_from_slice, Error, Item};

use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use tokio::{fs::File, io::AsyncReadExt};

pub(super) async fn read_cert(path: &Path) -> anyhow::Result<CertificateDer<'static>> {
    match read_one_async(path).await? {
        Item::X509Certificate(cert) => Ok(cert),
        item => anyhow::bail!("expected X.509 certificate, got {item:?}"),
    }
}

pub(super) async fn read_private_key(path: &Path) -> anyhow::Result<PrivateKeyDer<'static>> {
    match read_one_async(path).await? {
        Item::Pkcs1Key(key) => Ok(key.into()),
        Item::Pkcs8Key(key) => Ok(key.into()),
        Item::Sec1Key(key) => Ok(key.into()),
        item => anyhow::bail!("expected private key, got {item:?}"),
    }
}

async fn read_one_async(path: &Path) -> anyhow::Result<Item> {
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
    read_one_from_slice(&input)
        .map_err(|err| {
            let msg = match err {
                Error::MissingSectionEnd { end_marker } => format!(
                    "section end {:?} missing",
                    String::from_utf8_lossy(&end_marker)
                ),
                Error::IllegalSectionStart { line } => format!(
                    "illegal section start: {:?}",
                    String::from_utf8_lossy(&line)
                ),
                Error::Base64Decode(msg) => msg,
            };
            anyhow::anyhow!("failed to decode PEM file contents: {msg}")
        })?
        .ok_or_else(|| anyhow::anyhow!("no PEM section found in file contents"))
        .map(|(item, _)| item)
}
