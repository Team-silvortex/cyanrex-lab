use super::material::Material;
use std::{
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    os::unix::fs::{symlink, PermissionsExt},
};

fn load(_material: &Material, cert: &std::path::Path, key: &std::path::Path) -> bool {
    crate::serve_loopback::tls::load_tls_identity(cert, key, Ipv4Addr::LOCALHOST.into()).is_ok()
}

#[test]
fn tls_identity_accepts_owned_generated_certificate_and_matching_key_for_exact_ip() {
    let material = Material::new();
    let loaded = crate::serve_loopback::tls::load_tls_identity(
        &material.cert,
        &material.key,
        Ipv4Addr::LOCALHOST.into(),
    );
    drop(material);
    assert!(
        loaded.is_ok(),
        "owned generated TLS identity was not accepted"
    );
}

#[test]
fn tls_identity_accepts_ip_sans_bounded_chains_and_only_frozen_tls_options() {
    let material = Material::new();
    let cert = material.bytes(&material.cert);
    for ip in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let config =
            crate::serve_loopback::tls::load_tls_identity(&material.cert, &material.key, ip)
                .unwrap();
        assert!(config.alpn_protocols == [b"http/1.1".to_vec()]);
        assert_eq!(config.max_early_data_size, 0);
        assert_eq!(config.send_tls13_tickets, 0);
        assert!(!config.session_storage.can_cache());
        assert!(!config.ticketer.enabled());
        assert!(!config.key_log.will_log("CLIENT_RANDOM"));
    }
    for count in [1, 2, 8] {
        // Cardinality is checked here, not PKIX validity of this repeated synthetic chain.
        let chain = material.write(&format!("chain-{count}.pem"), &cert.repeat(count));
        assert!(load(&material, &chain, &material.key));
    }
    fs::set_permissions(&material.cert, fs::Permissions::from_mode(0o400)).unwrap();
    fs::set_permissions(&material.key, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(load(&material, &material.cert, &material.key));
}

#[test]
fn tls_identity_allows_lf_crlf_and_outer_whitespace_without_accepting_junk() {
    let material = Material::new();
    let cert = String::from_utf8(material.bytes(&material.cert)).unwrap();
    let key = String::from_utf8(material.bytes(&material.key)).unwrap();
    for (index, (cert, key)) in [
        (cert.clone(), key.clone()),
        (cert.replace('\n', "\r\n"), key.replace('\n', "\r\n")),
        (format!(" \t\r\n{cert}\n \t"), format!("\n\t{key} \r\n")),
    ]
    .into_iter()
    .enumerate()
    {
        let cert = material.write(&format!("certificate-{index}.pem"), cert.as_bytes());
        let key = material.write(&format!("private-{index}.pem"), key.as_bytes());
        assert!(load(&material, &cert, &key));
    }
}

#[test]
fn tls_identity_rejects_extra_labels_comments_multiple_keys_and_broken_frames() {
    let material = Material::new();
    let cert = material.bytes(&material.cert);
    let key = material.bytes(&material.key);
    let mut rejected = Vec::new();
    for (index, bytes) in [
        Vec::new(),
        b" \t\r\n".to_vec(),
        [b"junk\n".as_slice(), &cert].concat(),
        [cert.as_slice(), b"\ntrailing junk"].concat(),
        [b"# comment\n".as_slice(), &cert].concat(),
        [cert.as_slice(), key.as_slice()].concat(),
        key.clone(),
        b"-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----\n".to_vec(),
        b"-----BEGIN CERTIFICATE-----\n*invalid*\n-----END CERTIFICATE-----\n".to_vec(),
        String::from_utf8(cert.clone())
            .unwrap()
            .replace("END CERTIFICATE", "END PRIVATE KEY")
            .into_bytes(),
        [cert.as_slice(), &vec![b' '; 16 * 1024]].concat(),
        cert.repeat(9),
    ]
    .iter()
    .enumerate()
    {
        let path = material.write(&format!("bad-cert-{index}.pem"), bytes);
        rejected.push(!load(&material, &path, &material.key));
    }
    for (index, bytes) in [
        Vec::new(),
        [b"junk\n".as_slice(), &key].concat(),
        [key.as_slice(), b"trailing"].concat(),
        [key.as_slice(), key.as_slice()].concat(),
        [key.as_slice(), cert.as_slice()].concat(),
        cert.clone(),
        b"-----BEGIN PRIVATE KEY-----\n-----END PRIVATE KEY-----\n".to_vec(),
        String::from_utf8(key.clone())
            .unwrap()
            .replace("BEGIN PRIVATE KEY", "BEGIN ENCRYPTED PRIVATE KEY")
            .into_bytes(),
        [key.as_slice(), &vec![b' '; 16 * 1024]].concat(),
    ]
    .iter()
    .enumerate()
    {
        let path = material.write(&format!("bad-key-{index}.pem"), bytes);
        rejected.push(!load(&material, &material.cert, &path));
    }
    drop(material);
    assert!(rejected.into_iter().all(|value| value));
}

#[test]
fn tls_identity_accepts_matching_pkcs8_sec1_and_rsa_pkcs1_without_key_fallback() {
    let material = Material::new();
    assert!(load(&material, &material.cert, &material.key));
    material.run(&["ec", "-in", "server.key", "-out", "sec1.key"]);
    let sec1 = material.root.join("sec1.key");
    fs::set_permissions(&sec1, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(load(&material, &material.cert, &sec1));
    material.run(&["genrsa", "-traditional", "-out", "rsa.key", "2048"]);
    material.run(&[
        "req",
        "-new",
        "-x509",
        "-key",
        "rsa.key",
        "-out",
        "rsa.pem",
        "-days",
        "1",
        "-subj",
        "/CN=Disposable RSA Test",
        "-addext",
        "subjectAltName=IP:127.0.0.1",
    ]);
    let rsa = material.root.join("rsa.key");
    let cert = material.root.join("rsa.pem");
    for path in [&rsa, &cert] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    assert!(load(&material, &cert, &rsa));
    material.run(&[
        "pkcs8",
        "-topk8",
        "-in",
        "server.key",
        "-out",
        "encrypted.key",
        "-passout",
        "pass:synthetic-fixture-only",
    ]);
    let encrypted = material.root.join("encrypted.key");
    fs::set_permissions(&encrypted, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(!load(&material, &material.cert, &encrypted));
}

#[test]
fn tls_identity_refuses_mismatched_private_keys_and_exact_san_mismatches() {
    let material = Material::new();
    let other = Material::new();
    let wrong_key = crate::serve_loopback::tls::load_tls_identity(
        &material.cert,
        &other.key,
        Ipv4Addr::LOCALHOST.into(),
    )
    .is_err();
    let wrong_ip = crate::serve_loopback::tls::load_tls_identity(
        &material.cert,
        &material.key,
        Ipv4Addr::new(127, 0, 0, 2).into(),
    )
    .is_err();
    material.write("wrong.ext",b"subjectAltName=IP:127.0.0.2,DNS:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\n");
    material.run(&[
        "x509",
        "-req",
        "-in",
        "server.csr",
        "-CA",
        "ca.pem",
        "-CAkey",
        "ca.key",
        "-CAcreateserial",
        "-out",
        "wrong.pem",
        "-days",
        "1",
        "-extfile",
        "wrong.ext",
    ]);
    let wrong = material.root.join("wrong.pem");
    fs::set_permissions(&wrong, fs::Permissions::from_mode(0o600)).unwrap();
    let san_rejected = crate::serve_loopback::tls::load_tls_identity(
        &wrong,
        &material.key,
        Ipv4Addr::LOCALHOST.into(),
    )
    .is_err();
    let positive = crate::serve_loopback::tls::load_tls_identity(
        &wrong,
        &material.key,
        Ipv4Addr::new(127, 0, 0, 2).into(),
    )
    .is_ok();
    drop(other);
    drop(material);
    assert!(wrong_key && wrong_ip && san_rejected && positive);
}

#[test]
fn tls_identity_refuses_unsafe_files_parents_aliases_and_special_files_without_repair() {
    let material = Material::new();
    let cert = material.bytes(&material.cert);
    let key = material.bytes(&material.key);
    let mut rejected = Vec::new();
    for is_cert in [true, false] {
        let good = if is_cert {
            &material.cert
        } else {
            &material.key
        };
        let bytes = if is_cert { &cert } else { &key };
        let evaluate = |path: &std::path::Path| {
            if is_cert {
                load(&material, path, &material.key)
            } else {
                load(&material, &material.cert, path)
            }
        };
        for mode in [0o644, 0o640, 0o700] {
            let path = material.write(&format!("mode-{is_cert}-{mode}"), bytes);
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            rejected.push(!evaluate(&path));
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
                mode
            );
        }
        let link = material.root.join(format!("link-{is_cert}"));
        symlink(good, &link).unwrap();
        rejected.push(!evaluate(&link));
        let hard = material.root.join(format!("hard-{is_cert}"));
        fs::hard_link(good, &hard).unwrap();
        rejected.push(!evaluate(&hard));
        rejected.push(!evaluate(good));
        fs::remove_file(&hard).unwrap();
        let fifo = material.root.join(format!("fifo-{is_cert}"));
        let raw = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
        rejected.push(!evaluate(&fifo));
        let broad = material.root.join(format!("broad-{is_cert}"));
        fs::create_dir(&broad).unwrap();
        fs::set_permissions(&broad, fs::Permissions::from_mode(0o750)).unwrap();
        let path = broad.join("identity.pem");
        fs::write(&path, bytes).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        rejected.push(!evaluate(&path));
        rejected.push(!evaluate(&material.root));
        rejected.push(!evaluate(&material.root.join("missing")));
        rejected.push(!evaluate(
            &material.root.join(".").join(good.file_name().unwrap()),
        ));
        rejected.push(!evaluate(std::path::Path::new("relative.pem")));
    }
    assert!(load(&material, &material.cert, &material.key));
    drop(material);
    assert!(rejected.into_iter().all(|value| value));
}
