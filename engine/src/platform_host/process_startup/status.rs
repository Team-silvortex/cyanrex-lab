use super::{PlatformProcessIdentity, PlatformProcessStartupError as Error, Result};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Read,
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
};

const MAX_STATUS_BYTES: usize = 16 * 1024;
const FIELDS: [&str; 12] = [
    "Uid",
    "Gid",
    "Groups",
    "Pid",
    "Tgid",
    "Threads",
    "CapInh",
    "CapPrm",
    "CapEff",
    "CapAmb",
    "CapBnd",
    "NoNewPrivs",
];

#[derive(Clone)]
pub(super) struct Status {
    pub(super) uid: [u32; 4],
    pub(super) gid: [u32; 4],
    pub(super) groups: Vec<u32>,
    pub(super) capabilities: [u64; 4],
    pub(super) pid: u32,
    tgid: u32,
    pub(super) threads: u32,
    pub(super) nnp: u32,
}

fn decimal(value: &str) -> Result<u32> {
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Unavailable);
    }
    value.parse().map_err(|_| Error::Unavailable)
}

impl Status {
    pub(super) fn parse(text: &str) -> Result<Self> {
        if text.len() > MAX_STATUS_BYTES {
            return Err(Error::Unavailable);
        }
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            if let Some((name, value)) = line.split_once(':') {
                if FIELDS.contains(&name) && fields.insert(name, value.trim()).is_some() {
                    return Err(Error::Unavailable);
                }
            }
        }
        let field = |name| fields.get(name).copied().ok_or(Error::Unavailable);
        let ids = |name| -> Result<[u32; 4]> {
            field(name)?
                .split_ascii_whitespace()
                .take(5)
                .map(decimal)
                .collect::<Result<Vec<_>>>()?
                .try_into()
                .map_err(|_| Error::Unavailable)
        };
        let hex = |name| -> Result<u64> {
            let value = field(name)?;
            if value.is_empty() || value.len() > 16 || !value.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(Error::Unavailable);
            }
            u64::from_str_radix(value, 16).map_err(|_| Error::Unavailable)
        };
        let groups = field("Groups")?
            .split_ascii_whitespace()
            .take(129)
            .map(decimal)
            .collect::<Result<Vec<_>>>()?;
        if groups.len() > 128 {
            return Err(Error::Unavailable);
        }
        // CapBnd is parsed for a complete bounded record, not required to be zero. It is not
        // a current capability and cannot substitute for checking P/E/I/A plus NNP.
        hex("CapBnd")?;
        Ok(Self {
            uid: ids("Uid")?,
            gid: ids("Gid")?,
            groups,
            capabilities: [
                hex("CapInh")?,
                hex("CapPrm")?,
                hex("CapEff")?,
                hex("CapAmb")?,
            ],
            pid: decimal(field("Pid")?)?,
            tgid: decimal(field("Tgid")?)?,
            threads: decimal(field("Threads")?)?,
            nnp: decimal(field("NoNewPrivs")?)?,
        })
    }

    pub(super) fn validate(
        &self,
        identity: &PlatformProcessIdentity,
        pid: u32,
        tid: u32,
        single_main: bool,
        require_nnp: bool,
    ) -> Result<()> {
        if self.uid != [identity.uid; 4]
            || self.gid != [identity.gid; 4]
            || self.groups != identity.groups
            || self.capabilities != [0; 4]
            || self.tgid != pid
            || self.pid != tid
            || pid == 0
            || tid == 0
            || self.threads == 0
            || (single_main && (self.threads != 1 || self.pid != self.tgid))
            || self.nnp > 1
            || (require_nnp && self.nnp != 1)
        {
            return Err(Error::UnsafeProcess);
        }
        Ok(())
    }
}

pub(super) fn nnp_value(raw: i32) -> Result<u32> {
    match raw {
        0 => Ok(0),
        1 => Ok(1),
        _ => Err(Error::Unavailable),
    }
}

pub(super) fn tid() -> u32 {
    // SAFETY: gettid has no arguments or memory effects.
    unsafe { libc::gettid() as u32 }
}

pub(super) fn read() -> Result<Status> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open("/proc/thread-self/status")
        .map_err(|_| Error::Unavailable)?;
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
    // SAFETY: the held descriptor is valid and fstatfs initializes the entire output on success.
    if unsafe { libc::fstatfs(file.as_raw_fd(), filesystem.as_mut_ptr()) } != 0 {
        return Err(Error::Unavailable);
    }
    // SAFETY: the previous syscall succeeded.
    if unsafe { filesystem.assume_init() }.f_type as u64 != libc::PROC_SUPER_MAGIC as u64 {
        return Err(Error::Unavailable);
    }
    let mut bytes = Vec::new();
    file.take((MAX_STATUS_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable)?;
    let state = Status::parse(std::str::from_utf8(&bytes).map_err(|_| Error::Unavailable)?)?;
    let (mut real_uid, mut effective_uid, mut saved_uid) = (0, 0, 0);
    let (mut real_gid, mut effective_gid, mut saved_gid) = (0, 0, 0);
    let mut groups = [0 as libc::gid_t; 128];
    // SAFETY: each syscall receives valid, correctly sized writable storage. getgroups cannot
    // write more than 128 entries; larger group sets fail closed without allocating unbounded data.
    let (uids, gids, count, nnp) = unsafe {
        (
            libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid),
            libc::getresgid(&mut real_gid, &mut effective_gid, &mut saved_gid),
            libc::getgroups(128, groups.as_mut_ptr()),
            libc::prctl(
                libc::PR_GET_NO_NEW_PRIVS,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ),
        )
    };
    if uids != 0
        || gids != 0
        || !(0..=128).contains(&count)
        || state.uid[..3] != [real_uid, effective_uid, saved_uid]
        || state.gid[..3] != [real_gid, effective_gid, saved_gid]
        || state.groups != groups[..count.max(0) as usize]
        || state.nnp != nnp_value(nnp)?
    {
        return Err(Error::Unavailable);
    }
    Ok(state)
}
