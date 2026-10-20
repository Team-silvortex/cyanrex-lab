//! Explicit diagnostics and a restricted loopback Session host; no Engine or installer.

#[cfg(target_os = "linux")]
mod config;
#[cfg(target_os = "linux")]
mod private_input;
#[cfg(target_os = "linux")]
mod serve_loopback;
#[cfg(target_os = "linux")]
mod storage_check;

#[cfg(target_os = "linux")]
enum Command {
    Process,
    Config(std::path::PathBuf),
    Storage(std::path::PathBuf),
    ServeLoopback(std::path::PathBuf, std::net::SocketAddr, bool),
    ServeLoopbackTls(
        std::path::PathBuf,
        std::net::SocketAddr,
        std::path::PathBuf,
        std::path::PathBuf,
        bool,
    ),
}

#[cfg(target_os = "linux")]
fn identity() -> Result<
    (
        cyanrex_engine::platform_host::PlatformProcessIdentity,
        Command,
    ),
    (),
> {
    use cyanrex_engine::platform_host::PlatformProcessIdentity;
    let args: Vec<_> = std::env::args_os().skip(1).take(17).collect();
    if !matches!(args.len(), 7 | 9 | 11 | 12 | 15 | 16)
        || args.iter().take(7).any(|arg| arg.len() > 2048)
        || args.iter().skip(7).any(|arg| arg.len() > 4096)
    {
        return Err(());
    }
    let text: Vec<_> = args
        .iter()
        .map(|arg| arg.to_str().ok_or(()))
        .collect::<Result<_, _>>()?;
    let notify_systemd = text.last() == Some(&"--notify-systemd");
    let shape = text.len() - usize::from(notify_systemd);
    let input = match (text[0], shape) {
        ("check-process", 7) => Command::Process,
        (mode @ ("check-config" | "check-storage"), 9)
            if text[7] == "--config" && std::path::Path::new(text[8]).is_absolute() =>
        {
            let path = std::path::PathBuf::from(text[8]);
            if mode == "check-config" {
                Command::Config(path)
            } else {
                Command::Storage(path)
            }
        }
        ("serve-loopback", 11)
            if text[7] == "--config"
                && std::path::Path::new(text[8]).is_absolute()
                && text[9] == "--bind" =>
        {
            Command::ServeLoopback(
                std::path::PathBuf::from(text[8]),
                serve_loopback::parse_bind(text[10])?,
                notify_systemd,
            )
        }
        ("serve-loopback-tls", 15)
            if text[7] == "--config"
                && std::path::Path::new(text[8]).is_absolute()
                && text[9] == "--bind"
                && text[11] == "--cert"
                && std::path::Path::new(text[12]).is_absolute()
                && text[13] == "--key"
                && std::path::Path::new(text[14]).is_absolute() =>
        {
            Command::ServeLoopbackTls(
                std::path::PathBuf::from(text[8]),
                serve_loopback::parse_bind(text[10])?,
                std::path::PathBuf::from(text[12]),
                std::path::PathBuf::from(text[14]),
                notify_systemd,
            )
        }
        _ => return Err(()),
    };
    if text[1] != "--uid" || text[3] != "--gid" || text[5] != "--groups" {
        return Err(());
    }
    let number = |value: &str| -> Result<u32, ()> {
        if value.is_empty()
            || value.len() > 10
            || value.starts_with('0')
            || !value.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(());
        }
        value.parse().map_err(|_| ())
    };
    let groups = if text[6] == "none" {
        Vec::new()
    } else {
        text[6]
            .split(',')
            .take(129)
            .map(number)
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok((
        PlatformProcessIdentity::new(number(text[2])?, number(text[4])?, &groups)
            .map_err(|_| ())?,
        input,
    ))
}

#[cfg(target_os = "linux")]
fn check(
    identity: &cyanrex_engine::platform_host::PlatformProcessIdentity,
    input: &Command,
) -> Result<(), ()> {
    use cyanrex_engine::platform_host::prepare_platform_process;
    // Both calls precede the first worker. Repeating preparation must preserve the same policy.
    let checked = prepare_platform_process(identity).map_err(|_| ())?;
    prepare_platform_process(identity).map_err(|_| ())?;
    checked.verify_before_runtime().map_err(|_| ())?;
    if let Command::ServeLoopback(path, bind, notify_systemd) = input {
        return serve_loopback::run(path, *bind, &checked, *notify_systemd);
    }
    if let Command::ServeLoopbackTls(path, bind, cert, key, notify_systemd) = input {
        return serve_loopback::run_tls(path, *bind, cert, key, &checked, *notify_systemd);
    }
    if let Command::Config(path) = input {
        let result = config::check(path);
        checked.verify_before_runtime().map_err(|_| ())?;
        return result;
    }
    if let Command::Storage(path) = input {
        let result = config::load(path);
        checked.verify_before_runtime().map_err(|_| ())?;
        return storage_check::check(result?, &checked);
    }
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .spawn_scoped(scope, || checked.verify_current_thread())
            .map_err(|_| ())?
            .join()
            .map_err(|_| ())?
            .map_err(|_| ())
    })?;
    checked.verify_current_thread().map_err(|_| ())
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        let (identity, input) = match identity() {
            Ok(value) => value,
            Err(()) => {
                eprintln!(
                    "usage: cyanrex-platform check-process|check-config|check-storage|serve-loopback|serve-loopback-tls --uid N --gid N --groups GID,...|none [--config /absolute/input.json [--bind LOOPBACK:PORT [--cert ABS --key ABS] [--notify-systemd]]]"
                );
                std::process::exit(64);
            }
        };
        if check(&identity, &input).is_err() {
            match input {
                Command::Process => eprintln!("platform process check failed; no host started"),
                Command::Config(_) => {
                    eprintln!("platform configuration check failed; no host started")
                }
                Command::Storage(_) => eprintln!("platform storage check failed; no host started"),
                Command::ServeLoopback(..) | Command::ServeLoopbackTls(..) => {
                    eprintln!("platform loopback host failed")
                }
            }
            std::process::exit(1);
        }
        match input {
            Command::Process => println!("platform process check passed; no host started"),
            Command::Config(_) => println!("platform configuration check passed; no host started"),
            Command::Storage(_) => {
                println!("platform storage observations completed; no host started")
            }
            Command::ServeLoopback(..) | Command::ServeLoopbackTls(..) => {
                println!("platform loopback host stopped")
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("platform process check requires Linux; no host started");
        std::process::exit(1);
    }
}
