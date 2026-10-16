//! Borrowed raw query mechanics only; adapters retain their exact inventory and typed projection.
use super::errors::HttpError;

pub(super) const MAX_QUERY_BYTES: usize = 512;

pub(super) fn parse<'a, const N: usize>(
    query: Option<&'a str>,
    names: [&str; N],
) -> Result<[&'a str; N], HttpError> {
    let query = query.ok_or_else(HttpError::invalid_request)?;
    if query.is_empty() || query.len() > MAX_QUERY_BYTES || query.contains(['%', '+']) {
        return Err(HttpError::invalid_request());
    }
    // Values must be nonempty, so an empty slot unambiguously means not yet supplied.
    let mut fields = [""; N];
    for pair in query.split('&') {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(HttpError::invalid_request)?;
        if value.is_empty() || value.contains('=') {
            return Err(HttpError::invalid_request());
        }
        let index = names
            .iter()
            .position(|expected| *expected == name)
            .ok_or_else(HttpError::invalid_request)?;
        if !fields[index].is_empty() {
            return Err(HttpError::invalid_request());
        }
        fields[index] = value;
    }
    if fields.iter().any(|value| value.is_empty()) {
        return Err(HttpError::invalid_request());
    }
    Ok(fields)
}

pub(super) fn decimal(value: &str) -> Result<u64, HttpError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(HttpError::invalid_request());
    }
    value.parse().map_err(|_| HttpError::invalid_request())
}

#[cfg(test)]
mod tests;
