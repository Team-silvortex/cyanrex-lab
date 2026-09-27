use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::ContractError;

macro_rules! stable_ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Uuid);

        impl $name {
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl TryFrom<Uuid> for $name {
            type Error = ContractError;

            fn try_from(value: Uuid) -> Result<Self, Self::Error> {
                if value.is_nil() {
                    return Err(ContractError("ID must not be nil"));
                }
                Ok(Self(value))
            }
        }

        impl FromStr for $name {
            type Err = ContractError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let error = ContractError("ID must be a non-nil canonical lowercase hyphenated UUID");
                let parsed = Uuid::parse_str(value).map_err(|_| error.clone())?;
                if parsed.is_nil() || parsed.to_string() != value {
                    return Err(error);
                }
                Ok(Self(parsed))
            }
        }

        impl TryFrom<String> for $name {
            type Error = ContractError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                value.parse()
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.to_string()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    )+};
}

stable_ids!(
    AuthorityId,
    PrincipalId,
    WorkspaceId,
    ArtifactId,
    ArtifactRevisionId,
    TaskId,
    RunId,
    ReviewId,
    EventId,
    CorrelationId,
    LegacyAccountId,
    PolicyCommandId,
    IdentityCommandId,
);

macro_rules! validated_text {
    ($name:ident, $message:literal, $valid:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = ContractError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if !($valid)(value) {
                    return Err(ContractError($message));
                }
                Ok(Self(value.to_owned()))
            }
        }

        impl TryFrom<String> for $name {
            type Error = ContractError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                value.parse()
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

validated_text!(
    Sha256Digest,
    "SHA-256 must contain exactly 64 lowercase hexadecimal characters",
    |value: &str| value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
);

validated_text!(
    QualifiedName,
    "name must be a namespaced lowercase identifier of at most 128 bytes",
    |value: &str| value.len() <= 128
        && value.contains('.')
        && value.split('.').all(|part| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'_' | b'-')
                })
        })
);

// Import already-normalized server identities. Do not silently merge or rename snapshot entries.
validated_text!(
    LegacyUsername,
    "legacy username must be canonical lowercase ASCII and contain 3 to 64 bytes",
    |value: &str| (3..=64).contains(&value.len())
        && value.bytes().all(|byte| byte.is_ascii_lowercase()
            || byte.is_ascii_digit()
            || matches!(byte, b'_' | b'-' | b'.'))
);

/// Exact JSON integers are required by Rust and JavaScript consumers alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct RevisionNumber(u64);

impl TryFrom<u64> for RevisionNumber {
    type Error = ContractError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if !(1..=9_007_199_254_740_991).contains(&value) {
            return Err(ContractError(
                "revision must be a positive JSON-safe integer",
            ));
        }
        Ok(Self(value))
    }
}

impl From<RevisionNumber> for u64 {
    fn from(value: RevisionNumber) -> Self {
        value.0
    }
}

/// Contract version, independent of the product's continuing 0.4.x version sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct CoreSchemaVersion;

impl TryFrom<u8> for CoreSchemaVersion {
    type Error = ContractError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value != 1 {
            return Err(ContractError("unsupported collaboration contract version"));
        }
        Ok(Self)
    }
}

impl From<CoreSchemaVersion> for u8 {
    fn from(_: CoreSchemaVersion) -> Self {
        1
    }
}
