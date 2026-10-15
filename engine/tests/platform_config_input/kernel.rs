use std::{fs, os::unix::process::CommandExt, process::Command};

#[derive(PartialEq, Eq)]
pub(super) struct Identity {
    pub(super) uid: [u32; 4],
    pub(super) gid: [u32; 4],
    pub(super) groups: Vec<u32>,
    caps: [u64; 5],
}

impl Identity {
    pub(super) fn current() -> Self {
        let status = fs::read_to_string("/proc/thread-self/status").unwrap();
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap()
                .trim()
                .to_owned()
        };
        let ids = |name: &str| {
            field(name)
                .split_whitespace()
                .map(|id| id.parse::<u32>().unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let mut groups = field("Groups:")
            .split_whitespace()
            .map(|id| id.parse().unwrap())
            .collect::<Vec<u32>>();
        groups.sort_unstable();
        Self {
            uid: ids("Uid:"),
            gid: ids("Gid:"),
            groups,
            caps: ["CapPrm:", "CapEff:", "CapInh:", "CapAmb:", "CapBnd:"]
                .map(|name| u64::from_str_radix(&field(name), 16).unwrap()),
        }
    }

    pub(super) fn safe_ids(&self) -> bool {
        self.uid[0] != 0
            && self.uid[0] != u32::MAX
            && self.uid.iter().all(|id| *id == self.uid[0])
            && self.gid[0] != 0
            && self.gid[0] != u32::MAX
            && self.gid.iter().all(|id| *id == self.gid[0])
            && self.groups.len() <= 128
            && self.groups.iter().all(|id| *id != 0 && *id != u32::MAX)
            && self.groups.windows(2).all(|pair| pair[0] < pair[1])
    }
}

pub(super) fn no_new_privileges() -> i32 {
    // SAFETY: PR_GET_NO_NEW_PRIVS reads only this calling thread's kernel flag.
    let value = unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) };
    assert!(matches!(value, 0 | 1));
    value
}

pub(super) fn clear_child_capabilities(command: &mut Command) {
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    struct Data {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    // SAFETY: the forked child executes only syscalls over stack-owned data before
    // exec. It reduces its own P/E/I/A sets, never IDs/groups or parent authority.
    unsafe {
        command.pre_exec(|| {
            let header = Header {
                version: 0x2008_0522,
                pid: 0,
            };
            let data = [
                Data {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
                Data {
                    effective: 0,
                    permitted: 0,
                    inheritable: 0,
                },
            ];
            if libc::syscall(libc::SYS_capset, &header as *const Header, data.as_ptr()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::prctl(
                libc::PR_CAP_AMBIENT,
                libc::PR_CAP_AMBIENT_CLEAR_ALL as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}
