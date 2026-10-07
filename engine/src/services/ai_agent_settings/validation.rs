use super::{AiAgentSettingsError::InvalidInput, Result};
use crate::models::ai_agent::AiAgentProfile;
use std::collections::HashSet;

pub(super) fn validate(default: &Option<String>, profiles: &[AiAgentProfile]) -> Result<()> {
    if profiles.len() > 16 {
        return Err(InvalidInput);
    }
    let mut ids = HashSet::new();
    for profile in profiles {
        if !identifier(&profile.id, false)
            || !ids.insert(&profile.id)
            || !text(&profile.name, 128)
            || !text(&profile.model, 128)
            || !base_url(&profile.base_url)
            || profile
                .credential_ref
                .as_ref()
                .is_some_and(|value| !identifier(value, true))
        {
            return Err(InvalidInput);
        }
    }
    if default
        .as_ref()
        .is_some_and(|id| !profiles.iter().any(|item| item.id == *id && item.enabled))
    {
        return Err(InvalidInput);
    }
    Ok(())
}

fn identifier(value: &str, upper: bool) -> bool {
    let letter = |byte: u8| {
        if upper {
            byte.is_ascii_uppercase()
        } else {
            byte.is_ascii_lowercase()
        }
    };
    !value.is_empty()
        && value.len() <= 64
        && letter(value.as_bytes()[0])
        && value.bytes().all(|byte| {
            letter(byte) || byte.is_ascii_digit() || byte == b'_' || (!upper && byte == b'-')
        })
}

fn text(value: &str, maximum: usize) -> bool {
    !value
        .trim_matches(|character: char| character.is_whitespace() || character == '\u{feff}')
        .is_empty()
        && value.chars().count() <= maximum
        && !value.chars().any(char::is_control)
}

fn base_url(value: &str) -> bool {
    if !text(value, 2048)
        || value
            .chars()
            .any(|character| character.is_whitespace() || character == '\u{feff}')
    {
        return false;
    }
    let Some((_, authority_and_path)) = value.split_once("://") else {
        return false;
    };
    let authority = authority_and_path.split('/').next().unwrap_or("");
    if authority.is_empty()
        || authority.contains('@')
        || value.contains('\\')
        || !valid_authority_port(authority)
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url
            .host_str()
            .is_none_or(|host| host.is_empty() || host.contains('*'))
    {
        return false;
    }
    match url.scheme() {
        "https" => true,
        "http" => {
            // Do not let URL normalization turn non-literal 127.1/decimal hosts into
            // an accepted literal loopback endpoint.
            let host = if authority.starts_with('[') {
                authority
                    .split_once(']')
                    .map(|(host, _)| &host[1..])
                    .unwrap_or("")
            } else {
                authority.split(':').next().unwrap_or("")
            };
            host.eq_ignore_ascii_case("localhost") || matches!(host, "127.0.0.1" | "::1")
        }
        _ => false,
    }
}

fn valid_authority_port(authority: &str) -> bool {
    let digits = |port: &str| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit());
    if authority.starts_with('[') {
        let Some((_, suffix)) = authority.split_once(']') else {
            return false;
        };
        suffix.is_empty() || suffix.strip_prefix(':').is_some_and(digits)
    } else {
        authority
            .split_once(':')
            .is_none_or(|(_, port)| digits(port))
    }
}
