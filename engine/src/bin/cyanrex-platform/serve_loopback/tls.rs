//! Explicit private server identity, never client trust, auto-generation or hot reload.
mod pem;

use rustls::{
    client::verify_server_name,
    pki_types::ServerName,
    server::{ClientHello, NoServerSessionStorage, ParsedCertificate, ResolvesServerCert},
    sign::CertifiedKey,
    ServerConfig,
};
use std::{fmt, net::IpAddr, path::Path, sync::Arc};

pub(super) fn load_tls_identity(
    cert: &Path,
    key: &Path,
    expected_ip: IpAddr,
) -> Result<Arc<ServerConfig>, ()> {
    let cert = pem::certificates(&crate::private_input::read(cert)?)?;
    let key = pem::private_key(&crate::private_input::read(key)?)?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let key = CertifiedKey::from_der(cert, key, &provider).map_err(|_| ())?;
    // from_der can accept an unknown consistency result; this host requires positive equality.
    key.keys_match().map_err(|_| ())?;
    let leaf =
        ParsedCertificate::try_from(key.end_entity_cert().map_err(|_| ())?).map_err(|_| ())?;
    verify_server_name(&leaf, &ServerName::IpAddress(expected_ip.into())).map_err(|_| ())?;
    // Parse every configured certificate, not just its PEM wrapper. This is NOT chain trust,
    // certificate-time, revocation or client verification of this supplied server identity.
    for certificate in &key.cert {
        ParsedCertificate::try_from(certificate).map_err(|_| ())?;
    }
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|_| ())?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(IpIdentity(Arc::new(key))));
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config.max_early_data_size = 0;
    config.send_half_rtt_data = false;
    config.send_tls13_tickets = 0;
    config.max_tls13_tickets = 0;
    config.session_storage = Arc::new(NoServerSessionStorage {});
    config.key_log = Arc::new(rustls::NoKeyLog {});
    config.enable_secret_extraction = false;
    Ok(Arc::new(config))
}

struct IpIdentity(Arc<CertifiedKey>);

impl fmt::Debug for IpIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PlatformLoopbackTlsIdentity")
    }
}

impl ResolvesServerCert for IpIdentity {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        // Valid IP clients omit SNI. Never select a different Origin using a peer's DNS name.
        if hello.server_name().is_some()
            || hello
                .alpn()
                .is_some_and(|mut protocols| !protocols.any(|value| value == b"http/1.1"))
        {
            return None;
        }
        Some(Arc::clone(&self.0))
    }
}

#[cfg(test)]
#[path = "tls/tests.rs"]
mod tests;
