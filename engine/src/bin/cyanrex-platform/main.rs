//! Explicit diagnostics and a restricted loopback Session host; no Engine or installer.

#[cfg(target_os = "linux")]
mod config;
#[cfg(target_os = "linux")]
mod private_input;
#[cfg(target_os = "linux")]
mod serve_loopback;
#[cfg(target_os = "linux")]
mod session_ui;
#[cfg(target_os = "linux")]
mod storage_check;

#[cfg(target_os = "linux")]
enum Command {
    Process,
    Config(std::path::PathBuf),
    Storage(std::path::PathBuf),
    ServeLoopback(std::path::PathBuf, std::net::SocketAddr, bool, bool, bool),
    ServeLoopbackTls(
        std::path::PathBuf,
        std::net::SocketAddr,
        std::path::PathBuf,
        std::path::PathBuf,
        bool,
        bool,
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
    let args: Vec<_> = std::env::args_os().skip(1).take(19).collect();
    if !matches!(args.len(), 7 | 9 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18)
        || args.iter().take(7).any(|arg| arg.len() > 2048)
        || args.iter().skip(7).any(|arg| arg.len() > 4096)
    {
        return Err(());
    }
    let text: Vec<_> = args
        .iter()
        .map(|arg| arg.to_str().ok_or(()))
        .collect::<Result<_, _>>()?;
    // Fixed suffix order; duplicates, aliases and diagnostic flags never reach private input.
    let session_ui = text.last() == Some(&"--session-ui");
    let without_ui = text.len() - usize::from(session_ui);
    let notify_systemd = text.get(without_ui - 1) == Some(&"--notify-systemd");
    let without_notify = without_ui - usize::from(notify_systemd);
    let task_text_publication = text.get(without_notify - 1) == Some(&"--task-text-publication");
    let shape = without_notify - usize::from(task_text_publication);
    if (task_text_publication || session_ui || notify_systemd)
        && !matches!(text[0], "serve-loopback" | "serve-loopback-tls")
    {
        return Err(());
    }
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
                task_text_publication,
                notify_systemd,
                session_ui,
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
                task_text_publication,
                notify_systemd,
                session_ui,
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
    if let Command::ServeLoopback(path, bind, task_text_publication, notify_systemd, session_ui) =
        input
    {
        return serve_loopback::run(
            path,
            *bind,
            &checked,
            *task_text_publication,
            *notify_systemd,
            *session_ui,
        );
    }
    if let Command::ServeLoopbackTls(
        path,
        bind,
        cert,
        key,
        task_text_publication,
        notify_systemd,
        session_ui,
    ) = input
    {
        return serve_loopback::run_tls(
            path,
            *bind,
            cert,
            key,
            &checked,
            *task_text_publication,
            *notify_systemd,
            *session_ui,
        );
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
                    "usage: cyanrex-platform check-process|check-config|check-storage|serve-loopback|serve-loopback-tls --uid N --gid N --groups GIDS|none [--config ABS [--bind LOOPBACK:PORT [--cert ABS --key ABS] [--task-text-publication] [--notify-systemd] [--session-ui]]]"
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
