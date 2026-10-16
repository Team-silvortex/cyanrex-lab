//! Pure shared Origin policy, independent of host preparation and HTTP adapters.
use std::net::IpAddr;

pub(crate) fn validate(value: &str) -> Result<(), ()> {
    let invalid = ();
    if value.len() > 2048 {
        return Err(invalid);
    }
    let url = reqwest::Url::parse(value).map_err(|_| invalid)?;
    if url.origin().ascii_serialization() != value
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid);
    }
    let host = url.host_str().ok_or(invalid)?;
    if host.contains('*') {
        return Err(invalid);
    }
    let loopback = host == "localhost"
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if url.scheme() == "https" || (url.scheme() == "http" && loopback) {
        Ok(())
    } else {
        Err(invalid)
    }
}
