use axum::{body::Body, http::StatusCode, response::Response, routing::get, Router};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) const SECRET: &str = "synthetic-deploy-password-never-print";

pub(super) fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cyanrex-release"));
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("CYANREX_")
            || name.starts_with("PG")
            || ["NODE_OPTIONS", "BASH_ENV", "ENV", "DATABASE_URL"].contains(&name.as_ref())
        {
            command.env_remove(key);
        }
    }
    command
}

pub(super) fn complete(command: &mut Command) -> Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("synthetic CLI exceeded twelve seconds");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child.wait_with_output().unwrap()
}

pub(super) struct Fixture {
    pub root: PathBuf,
    pub package: PathBuf,
}
impl Fixture {
    pub fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "cyanrex-deploy-health-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(root.join("bin")).unwrap();
        let docker = r#"#!/bin/bash
set -eu
printf '%s\n' "$*" >> "$TEST_DOCKER_CALLS"
case "${1:-}:${2:-}" in
 info:|rm:*) exit 0 ;;
 image:inspect)
   if [[ "$*" == *--format* ]]; then
     case "${*: -1}" in *engine:*) digit=1 ;; *frontend:*) digit=2 ;; postgres:*) digit=3 ;; *) exit 97 ;; esac
     printf 'sha256:%064d\n' "$digit"
   fi ;;
 save:*) while [ "$#" -gt 0 ]; do if [ "$1" = -o ]; then printf 'synthetic image\n' > "$2"; exit 0; fi; shift; done; exit 97 ;;
 create:*) printf 'synthetic-tool-container\n' ;;
 cp:*) /bin/cp "$TEST_RELEASE_BIN" "$3" ;;
 compose:version) exit 0 ;;
 compose:*)
   case " $* " in
     *' config --services '*) printf 'engine\nfrontend\npostgres\n' ;;
     *' up -d '*) exit "${TEST_UP_STATUS:-0}" ;;
     *' ps '*|*' logs -f '*|*' down '*) exit 0 ;;
     *) printf 'Unexpected fixture compose: %s\n' "$*" >&2; exit 97 ;;
   esac ;;
 *) printf 'Forbidden fixture Docker command: %s\n' "$*" >&2; exit 97 ;;
esac
"#;
        fs::write(root.join("bin/docker"), docker).unwrap();
        fs::set_permissions(root.join("bin/docker"), fs::Permissions::from_mode(0o700)).unwrap();
        // The pre-fix shell reports arbitrary HTTP success; only the explicit hang case uses curl.
        for name in ["wget", "curl"] {
            fs::write(root.join("bin").join(name), "#!/bin/bash\nif [ \"${TEST_REAL_HTTP:-0}\" = 1 ]; then exec /usr/bin/curl -q --noproxy '*' -fsS \"${@: -1}\"; fi\nprintf '<html>not a health response</html>\\n'\n").unwrap();
            fs::set_permissions(
                root.join("bin").join(name),
                fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }
        let mut pack = Command::new("/bin/bash");
        pack.arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../scripts/package-distribution.sh"),
        )
        .args([
            "--skip-checks",
            "--skip-build",
            "--no-compress",
            "--version",
            env!("CARGO_PKG_VERSION"),
            "--output",
        ])
        .arg(root.join("output"));
        configure(&mut pack, &root);
        let output = complete(&mut pack);
        success(&output);
        let package = fs::read_dir(root.join("output"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(root.join("calls.log"), "").unwrap();
        Self { root, package }
    }

    pub fn run(&self, args: &[&str], values: &[(&str, String)]) -> Output {
        fs::write(self.package.join(".env"), format!("POSTGRES_PASSWORD={SECRET}\nCYANREX_ADMIN_PASSWORD={SECRET}\nCYANREX_ADMIN_TOTP_SECRET=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n")).unwrap();
        let before = fs::read(self.package.join(".env")).unwrap();
        let mut deploy = Command::new("/bin/bash");
        deploy.arg(self.package.join("deploy.sh")).args(args);
        configure(&mut deploy, &self.root);
        for (key, value) in values {
            deploy.env(key, value);
        }
        let output = complete(&mut deploy);
        assert_eq!(fs::read(self.package.join(".env")).unwrap(), before);
        assert!(
            !text(&output).contains(SECRET),
            "deployment leaked synthetic credentials"
        );
        output
    }

    pub fn calls(&self) -> String {
        fs::read_to_string(self.root.join("calls.log")).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn configure(command: &mut Command, root: &std::path::Path) {
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("CYANREX_")
            || name.starts_with("PG")
            || name.starts_with("TEST_")
            || ["NODE_OPTIONS", "BASH_ENV", "ENV", "DATABASE_URL"].contains(&name.as_ref())
        {
            command.env_remove(key);
        }
    }
    command
        .env(
            "PATH",
            format!(
                "{}:{}",
                root.join("bin").display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .env("POSTGRES_IMAGE", "postgres:16")
        .env("TEST_DOCKER_CALLS", root.join("calls.log"))
        .env("TEST_RELEASE_BIN", env!("CARGO_BIN_EXE_cyanrex-release"));
}

pub(super) async fn server(
    body: &'static str,
    status: StatusCode,
    delay: Duration,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    let app = Router::new().route(
        "/health",
        get(move |request: axum::extract::Request| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                assert!(!request.headers().contains_key("cookie"));
                assert!(!request.headers().contains_key("authorization"));
                tokio::time::sleep(delay).await;
                Response::builder()
                    .status(status)
                    .header("location", "/redirect-must-not-be-followed")
                    .body(Body::from(body))
                    .unwrap()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, requests, handle)
}
pub(super) fn text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub(super) async fn streaming_server(
    chunks: Vec<(&'static str, Duration)>,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    let app = Router::new().route(
        "/health",
        get(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            let chunks = chunks.clone();
            async move {
                let stream =
                    futures_util::stream::unfold(chunks.into_iter(), |mut chunks| async move {
                        let (text, delay) = chunks.next()?;
                        tokio::time::sleep(delay).await;
                        Some((
                            Ok::<_, std::convert::Infallible>(axum::body::Bytes::from_static(
                                text.as_bytes(),
                            )),
                            chunks,
                        ))
                    });
                Response::builder()
                    .status(200)
                    .body(Body::from_stream(stream))
                    .unwrap()
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, requests, handle)
}

pub(super) async fn recovering_server(
    ready_delay: Duration,
) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();
    let app = Router::new().route(
        "/health",
        get(move || {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            async move {
                if attempt == 0 {
                    return "{\"status\":\"degraded\"}";
                }
                tokio::time::sleep(ready_delay).await;
                "{\"status\":\"ok\"}"
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, requests, handle)
}
pub(super) fn success(output: &Output) {
    assert!(output.status.success(), "{}", text(output));
}
