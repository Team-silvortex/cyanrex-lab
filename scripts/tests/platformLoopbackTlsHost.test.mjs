import { assertDocumentationReference } from "./helpers/documentation-navigation.mjs";
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);
const read = path => readFile(new URL(path, root), "utf8");
const code = source => source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
const compact = source => code(source).replace(/\s+/g, "");
const directory = "engine/src/bin/cyanrex-platform/serve_loopback";

// Structural guards supplement actual native TLS, capless CLI and disposable SQL tests.
test("TLS mode is fixed private CLI input and shares original startup without widening plain binding", async () => {
  const main = await read("engine/src/bin/cyanrex-platform/main.rs");
  for (const value of ["take(19)", "7 | 9 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18", '"serve-loopback-tls"', '"--notify-systemd"',
    'text[11] == "--cert"', 'text[13] == "--key"', "Path::new(text[12]).is_absolute()",
    "Path::new(text[14]).is_absolute()", "serve_loopback::parse_bind(text[10])", "serve_loopback::run_tls("])
    assert.ok(main.includes(value), value);
  assert.doesNotMatch(code(main), /tokio|rustls|TcpListener|PgPool|PgConnectOptions|open_platform_storage_expected|build_platform_(?:session|workspace)_router/);
  const source = await read(`${directory}/mod.rs`);
  const dense = compact(source);
  for (const value of ["run_with_transport(path,bind,process,None,task_text_publication,notify_systemd,session_ui)",
    "run_with_transport(path,bind,process,Some((cert,key)),task_text_publication,notify_systemd,session_ui)"])
    assert.ok(dense.includes(value) || dense.includes(value.slice(0, -1) + ",)"), value);
  for (const value of [
    'let scheme = if tls_paths.is_some() { "https" } else { "http" };',
    'input.config.origin() != format!("{scheme}://{bind}")', "tls::load_tls_identity(cert, key, bind.ip())",
    "http::serve_notified("])
    assert.ok(source.includes(value), value);
  const sequence = ["let loaded = config::load(path)", "let checked = process.verify_before_runtime()",
    "let input = loaded?", "StorageExpectation::Markers(expected)", "let scheme =",
    "tls::load_tls_identity(", "let checked = process.verify_before_runtime()", "let identity = identity?",
    "inspect_platform_artifact_root(", "new_current_thread()", "open_platform_storage_expected(",
    "let checked = process.verify_current_thread()", "let storage = opened.map_err(",
    "PlatformWorkspaceHttpState::with_text_publication_capacities(", "SignalKind::terminate()", "SignalKind::interrupt()",
    "TcpListener::bind(", "process.verify_current_thread()", 'println!("platform loopback host ready")'];
  let offset = -1;
  for (const value of sequence) {
    const found = source.indexOf(value, offset + 1);
    assert.ok(found > offset && found >= 0, value);
    offset = found;
  }
  for (const path of ["engine/src/main.rs", "engine/src/state.rs", "engine/src/application.rs",
    "engine/openapi/openapi.json", "sdk-js/src/index.ts"])
    assert.doesNotMatch(await read(path), /serve-loopback-tls|load_tls_identity|serve_loopback/, path);
});

test("TLS material stays bounded private input with strict framing and positive key and IP checks", async () => {
  const tls = compact(await read(`${directory}/tls.rs`));
  for (const value of ["pem::certificates(&crate::private_input::read(cert)?)?",
    "pem::private_key(&crate::private_input::read(key)?)?", "rustls::crypto::ring::default_provider()",
    "CertifiedKey::from_der(", "key.keys_match().map_err(|_|())?", "ParsedCertificate::try_from(",
    "verify_server_name(&leaf,&ServerName::IpAddress(expected_ip.into()))", "forcertificatein&key.cert",
    "ServerConfig::builder_with_provider(provider)", ".with_no_client_auth()",
    'config.alpn_protocols=vec![b"http/1.1".to_vec()]', "hello.server_name().is_some()",
    "config.max_early_data_size=0", "config.send_half_rtt_data=false", "config.send_tls13_tickets=0",
    "NoServerSessionStorage", "rustls::NoKeyLog", "config.enable_secret_extraction=false"])
    assert.ok(tls.includes(value), value);
  assert.doesNotMatch(tls, /install_default|std::env|KeyLogFile|Command::|std::fs|pubfn|derive\((?:Debug|Clone|Serialize)/);
  const pem = compact(await read(`${directory}/tls/pem.rs`));
  for (const value of ["frames(bytes,false)?", "frames(bytes,true)?", "frames.len()>ifkey{1}else{8}",
    "byte.is_ascii_alphanumeric()", "bytes.trim_ascii_start()", "has_body", "frames.is_empty()",
    "CertificateDer::from_pem_slice(frame)", "PrivateKeyDer::from_pem_slice(frames[0])",
    "BEGINCERTIFICATE", "BEGINPRIVATEKEY", "BEGINRSAPRIVATEKEY", "BEGINECPRIVATEKEY"])
    assert.ok(pem.includes(value), value);
  assert.doesNotMatch(pem, /pem_slice_iter|filter_map|from_pem_file|std::env|Command::/);
  const manifest = await read("engine/Cargo.toml");
  assert.match(manifest, /^rustls = \{ version = "=0\.23\.46", default-features = false, features = \["ring", "std", "tls12"\] \}/m);
  assert.match(manifest, /^tokio-rustls = \{ version = "=0\.26\.4", default-features = false, features = \["ring", "tls12"\] \}/m);
  assert.doesNotMatch(manifest, /^rustls-pemfile\s*=/m);
});

test("TLS uses the original connection permit with only a handshake timeout before generic HTTP", async () => {
  const http = compact(await read(`${directory}/http.rs`));
  for (const value of ["serve_notified(listener,router,authority,Some(config),None,stop).await",
    "serve_notified(listener,router,authority,None,None,stop).await", "try_acquire_owned()",
    "super::handshake::accept(stream,config,&mutshutdown).await", "drive_http(stream,router,shutdown,permit).await",
    "I:AsyncRead+AsyncWrite+Unpin+Send+'static"])
    assert.ok(http.includes(value), value);
  assert.ok(http.indexOf("try_acquire_owned()") < http.indexOf("connections.spawn("));
  assert.equal([...http.matchAll(/tokio::time::timeout\(/g)].length, 1, "only the original soft drain wraps active work");
  const handshake = compact(await read(`${directory}/handshake.rs`));
  for (const value of ["Instant::now()+Duration::from_secs(2)", "TlsAcceptor::from(config)",
    "biased;", "_=shutdown.changed()=>None", "tokio::time::timeout_at(deadline,acceptor.accept(stream))",
    "session.server_name().is_some()", 'None|Some(b"http/1.1")'])
    assert.ok(handshake.includes(value), value);
  assert.equal([...handshake.matchAll(/timeout_at\(/g)].length, 1);
  assert.doesNotMatch(handshake, /next\.run|Router|build_platform|Session|retry|std::env|Command::/);
});

test("CI selects every exact native TLS host SQL boundary and documentation keeps PKI and LAN separate", async () => {
  const cases = [];
  for (const file of await readdir(new URL("engine/tests/platform_loopback_tls_host/", root))) {
    if (!file.endsWith(".rs")) continue;
    for (const [, name] of (await read(`engine/tests/platform_loopback_tls_host/${file}`))
      .matchAll(/async fn (postgres_platform_loopback_(?:tls_)?host_\w+)\(/g))
      cases.push(`${file.slice(0, -3)}::${name}`);
  }
  assert.equal(cases.length, 8, "new SQL boundaries require deliberate exact selection");
  const runner = await read("scripts/test-platform-loopback-tls-host.sh");
  const selected = [...runner.matchAll(/^  (\w+::postgres_platform_loopback_(?:tls_)?host_\w+)\b/gm)].map(match => match[1]);
  assert.deepEqual(selected.sort(), cases.sort());
  for (const pattern of [/CYANREX_TEST_DATABASE_URL:\?/, /command -v openssl/, /--locked --test platform_loopback_tls_host_tdd/,
    /--ignored --list/, /grep -Fx/, /--ignored --exact/]) assert.match(runner, pattern);
  assert.doesNotMatch(runner, /\bDATABASE_URL=|initdb|pg_ctl|start\.sh/);
  const ci = await read(".github/workflows/ci.yml");
  assert.match(ci, /&& bash scripts\/test-platform-loopback-tls-host\.sh/);
  assert.match(ci, /command -v openssl >\/dev\/null && cargo fmt/);
  for (const locale of ["en", "zh-CN"]) {
    const guide = await read(`docs/${locale}/platform-loopback-tls-host.md`);
    for (const value of ["issues/3", "serve-loopback-tls", "--cert", "--key", "16 KiB", "PKCS8", "PKCS1",
      "SEC1", "SAN", "SNI", "ALPN", "EKU", "Secure", "OpenSSL", "CYANREX_TEST_DATABASE_URL"])
      assert.ok(guide.includes(value), `${locale}: ${value}`);
    assert.match(guide, locale === "en" ? /not isolate cookies by port/ : /不提供 Cookie 端口隔离/);
    assert.match(guide, locale === "en" ? /not a complete PKI check/ : /不是完整 PKI 校验/);
    assert.equal(guide, await read(`frontend/public/course/${locale}/platform-loopback-tls-host.md`));
    for (const file of ["README.md", "platform-loopback-host.md", "platform-session-router.md", "architecture.md",
      "architecture-map.md", "platform-network.md", "security.md", "capability-maturity.md", "testing-guide.md"])
      await assertDocumentationReference(`docs/${locale}/${file}`, `docs/${locale}/platform-loopback-tls-host.md`);
  }
});
