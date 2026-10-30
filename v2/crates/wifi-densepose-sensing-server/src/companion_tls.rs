//! Private, persistent TLS identity for the QR-pinned companion listener.
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io, path::Path, sync::Arc};
#[derive(Serialize, Deserialize)]
struct DiskIdentity {
    installation_id: String,
    certificate: Vec<u8>,
    private_key: Vec<u8>,
}
// Publish a complete first identity without replacing another process's identity.
fn create_identity_exclusively(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension(format!("create-{}", super::companion::secret()));
    super::companion::atomic_private_write(&temporary, bytes)?;
    let result = std::fs::hard_link(&temporary, path);
    let cleanup = std::fs::remove_file(&temporary);
    result?;
    cleanup?;
    std::fs::File::open(
        path.parent()
            .ok_or_else(|| io::Error::other("identity parent"))?,
    )?
    .sync_all()
}
pub struct Identity {
    pub installation_id: String,
    pub cert_sha256: String,
    pub config: Arc<rustls::ServerConfig>,
}
impl Identity {
    pub fn load_or_create(dir: &Path, preferred_installation_id: Option<&str>) -> io::Result<Self> {
        super::companion::private_dir(dir)?;
        if preferred_installation_id.is_some_and(|id| {
            id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) {
            return Err(io::Error::other("invalid configured installation"));
        }
        let path = dir.join("companion-identity.json");
        let data: DiskIdentity = if path.exists() {
            serde_json::from_slice(&super::companion::private_read(&path)?)
                .map_err(io::Error::other)?
        } else {
            let cert = rcgen::generate_simple_self_signed(vec!["ruview.local".into()])
                .map_err(io::Error::other)?;
            let data = DiskIdentity {
                installation_id: preferred_installation_id
                    .map(str::to_owned)
                    .unwrap_or_else(super::companion::secret),
                certificate: cert.serialize_der().map_err(io::Error::other)?,
                private_key: cert.serialize_private_key_der(),
            };
            create_identity_exclusively(
                &path,
                &serde_json::to_vec(&data).map_err(io::Error::other)?,
            )?;
            data
        };
        if data.installation_id.is_empty()
            || data.installation_id.len() > 128
            || preferred_installation_id.is_some_and(|id| id != data.installation_id)
        {
            return Err(io::Error::other("invalid companion installation"));
        }
        let pin = format!("{:x}", Sha256::digest(&data.certificate));
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(
            vec![CertificateDer::from(data.certificate)],
            PrivatePkcs8KeyDer::from(data.private_key).into(),
        )
        .map_err(io::Error::other)?;
        Ok(Self {
            installation_id: data.installation_id,
            cert_sha256: pin,
            config: Arc::new(config),
        })
    }
}
pub async fn serve(
    listener: tokio::net::TcpListener,
    config: Arc<rustls::ServerConfig>,
    app: axum::Router,
) -> io::Result<()> {
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    let slots = Arc::new(tokio::sync::Semaphore::new(64));
    loop {
        let (stream, _) = listener.accept().await?;
        let Ok(permit) = slots.clone().try_acquire_owned() else {
            continue;
        };
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let Ok(Ok(tls)) =
                tokio::time::timeout(std::time::Duration::from_secs(10), acceptor.accept(stream))
                    .await
            else {
                return;
            };
            let service = hyper_util::service::TowerToHyperService::new(app);
            let builder =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
            let _ = builder
                .serve_connection_with_upgrades(hyper_util::rt::TokioIo::new(tls), service)
                .await;
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_identity_and_configured_mismatch() {
        let d = tempfile::tempdir().unwrap();
        let first = Identity::load_or_create(d.path(), Some("mac-installation")).unwrap();
        let second = Identity::load_or_create(d.path(), Some("mac-installation")).unwrap();
        assert_eq!(first.cert_sha256, second.cert_sha256);
        assert_eq!(first.installation_id, "mac-installation");
        assert!(Identity::load_or_create(d.path(), Some("other")).is_err());
        std::fs::write(d.path().join("companion-identity.json"), b"corrupt").unwrap();
        assert!(Identity::load_or_create(d.path(), None).is_err());
    }
    #[test]
    fn first_identity_publication_never_replaces_existing() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("identity");
        create_identity_exclusively(&p, b"first").unwrap();
        assert!(create_identity_exclusively(&p, b"second").is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"first");
    }
}
