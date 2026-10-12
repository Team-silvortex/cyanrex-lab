use super::{FileChunkClientConfig, FileChunkClientError, FileChunkTransport};
use std::{net::IpAddr, time::Duration};

pub(super) fn configure(
    origin: &str,
    config: &FileChunkClientConfig,
) -> Result<(reqwest::Url, reqwest::Client), FileChunkClientError> {
    let invalid = FileChunkClientError::InvalidConfiguration;
    if !(1..=16).contains(&config.capacity)
        || !(Duration::from_secs(1)..=Duration::from_secs(60)).contains(&config.header_timeout)
        || !(Duration::from_secs(1)..=Duration::from_secs(60)).contains(&config.idle_timeout)
        || config.total_timeout < config.idle_timeout
        || config.total_timeout > Duration::from_secs(300)
        || origin.len() > 2048
    {
        return Err(invalid);
    }
    let mut endpoint =
        reqwest::Url::parse(origin).map_err(|_| FileChunkClientError::InvalidConfiguration)?;
    if endpoint.origin().ascii_serialization() != origin
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.path() != "/"
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.host_str().is_none_or(|host| host.contains('*'))
    {
        return Err(invalid);
    }
    match config.transport {
        FileChunkTransport::Https if endpoint.scheme() == "https" => {}
        FileChunkTransport::LoopbackHttp
            if endpoint.scheme() == "http"
                && endpoint.host_str().is_some_and(|host| {
                    host.trim_start_matches('[')
                        .trim_end_matches(']')
                        .parse::<IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
                })
                && config.trusted_root_pem.is_none() => {}
        _ => return Err(invalid),
    }
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .https_only(config.transport == FileChunkTransport::Https)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .referer(false)
        .http1_only()
        .pool_max_idle_per_host(0)
        .connect_timeout(config.header_timeout);
    // No cookie provider/store, custom verifier, TLS identity or insecure TLS setting exists.
    // A global Client timeout would incorrectly continue into Receiver's separate body deadline.
    if let Some(pem) = &config.trusted_root_pem {
        if config.transport != FileChunkTransport::Https || !(1..=65536).contains(&pem.len()) {
            return Err(invalid);
        }
        // from_pem alone only wraps bytes with rustls-only builds; require a nonempty parsed
        // certificate bundle and let Client construction validate each DER trust anchor.
        let certificates = reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|_| FileChunkClientError::InvalidConfiguration)?;
        if certificates.is_empty() {
            return Err(invalid);
        }
        for certificate in certificates {
            builder = builder.add_root_certificate(certificate);
        }
    }
    let http = builder
        .build()
        .map_err(|_| FileChunkClientError::InvalidConfiguration)?;
    endpoint.set_path("/platform/v1/file-chunks");
    Ok((endpoint, http))
}
