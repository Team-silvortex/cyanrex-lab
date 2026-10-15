//! Explicit Linux pre-runtime preparation, never privilege dropping or a reusable sandbox proof.

mod status;
#[cfg(test)]
mod tests;

/// Operator-selected identity and exact supplementary-group allowlist. No discovered defaults.
#[derive(Clone)]
pub struct PlatformProcessIdentity {
    uid: u32,
    gid: u32,
    groups: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlatformProcessStartupError {
    #[error("invalid platform process identity")]
    InvalidIdentity,
    #[error("platform process startup state is unavailable")]
    Unavailable,
    #[error("platform process startup state is unsafe")]
    UnsafeProcess,
}

type Result<T> = std::result::Result<T, PlatformProcessStartupError>;

impl PlatformProcessIdentity {
    /// Explicit numeric operator policy, not discovered defaults or account-name lookup.
    pub fn new(uid: u32, gid: u32, groups: &[u32]) -> Result<Self> {
        if matches!(uid, 0 | u32::MAX)
            || matches!(gid, 0 | u32::MAX)
            || groups.len() > 128
            || groups.iter().any(|gid| matches!(*gid, 0 | u32::MAX))
            || groups.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(PlatformProcessStartupError::InvalidIdentity);
        }
        Ok(Self {
            uid,
            gid,
            groups: groups.to_vec(),
        })
    }
}

/// Initial preparation plus a fresh calling-thread verifier, not resource authorization.
/// Not Clone/Debug/serializable; moving it to another process does not preserve validity.
pub struct PlatformProcessStartup {
    identity: PlatformProcessIdentity,
    pid: u32,
}

/// Requires the single main thread, before runtime workers, secrets, databases or listeners.
/// Refuses unsafe identity/capabilities instead of dropping them. Sets only this thread's NNP;
/// that irreversible change may remain even if a subsequent check fails. Never auto-retry.
pub fn prepare_platform_process(
    identity: &PlatformProcessIdentity,
) -> Result<PlatformProcessStartup> {
    let pid = std::process::id();
    status::read()?.validate(identity, pid, status::tid(), true, false)?;
    // SAFETY: fixed prctl operation with documented scalar arguments. No credential/capability
    // setters are used; only the caller's no-new-privileges flag is irreversibly restricted.
    if unsafe {
        libc::prctl(
            libc::PR_SET_NO_NEW_PRIVS,
            1 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
            0 as libc::c_ulong,
        )
    } != 0
    {
        return Err(PlatformProcessStartupError::Unavailable);
    }
    status::read()?.validate(identity, pid, status::tid(), true, true)?;
    Ok(PlatformProcessStartup {
        identity: identity.clone(),
        pid,
    })
}

impl PlatformProcessStartup {
    /// Fresh single-main-thread check before reading startup input, never a historical proof
    /// that no worker, secret, connection or listener existed earlier.
    pub fn verify_before_runtime(&self) -> Result<()> {
        status::read()?.validate(&self.identity, self.pid, status::tid(), true, true)
    }

    /// A fresh calling-thread check, not continuous enforcement or a permission for later work.
    pub fn verify_current_thread(&self) -> Result<()> {
        status::read()?.validate(&self.identity, self.pid, status::tid(), false, true)
    }
}
