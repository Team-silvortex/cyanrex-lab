use super::{certificates::Certificates, support::*};
use std::{
    fs,
    net::{IpAddr, Ipv4Addr},
    os::unix::fs::{symlink, PermissionsExt},
};
#[test]
fn platform_tls_cli_rejects_unsafe_malformed_mismatched_and_wrong_san_materials_without_dial() {
    let (runs, no_db, no_ready, unchanged) = Fixture::checked(|f| {
        let db = listener();
        let bind = address();
        let root = root(f, "artifact-root");
        let config = input(
            f,
            "tls.json",
            &document(&root, db.local_addr().unwrap().port(), bind),
        );
        let good = Certificates::generate(&f.path("."), IpAddr::V4(Ipv4Addr::LOCALHOST));
        let before = good.bytes();
        let other = Certificates::generate(&f.path("."), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)));
        let cert = &good.certificate;
        let key = &good.key;
        let mut pairs = Vec::new();
        let missing = f.path("missing-cert");
        pairs.push((missing, key.clone()));
        let link = f.path("linked-cert");
        symlink(cert, &link).unwrap();
        pairs.push((link, key.clone()));
        let hard = f.write("hard-original", &before.0, 0o600);
        let hard_alias = f.path("hard-alias");
        fs::hard_link(&hard, &hard_alias).unwrap();
        pairs.push((hard, key.clone()));
        let broad = f.write("readable-cert", &before.0, 0o644);
        pairs.push((broad, key.clone()));
        let fifo_path = f.path("cert-fifo");
        fifo(&fifo_path);
        pairs.push((fifo_path, key.clone()));
        let directory = super::legacy::root(f, "cert-is-directory");
        pairs.push((directory, key.clone()));
        let parent = super::legacy::root(f, "broad-parent");
        let leaf = parent.join("cert.pem");
        fs::write(&leaf, &before.0).unwrap();
        fs::set_permissions(&leaf, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o750)).unwrap();
        pairs.push((leaf, key.clone()));
        for (index, bytes) in [
            Vec::new(),
            CANARY.as_bytes().to_vec(),
            vec![b'x'; 16385],
            before.0.repeat(9),
            [before.0.as_slice(), b"\nnot-pem-tail"].concat(),
            [before.0.as_slice(), before.1.as_slice()].concat(),
        ]
        .into_iter()
        .enumerate()
        {
            pairs.push((
                f.write(&format!("bad-cert-{index}"), &bytes, 0o600),
                key.clone(),
            ));
        }
        let keylink = f.path("linked-key");
        symlink(key, &keylink).unwrap();
        pairs.push((cert.clone(), keylink));
        let keyhard = f.write("hard-key-original", &before.1, 0o600);
        fs::hard_link(&keyhard, f.path("hard-key-alias")).unwrap();
        pairs.push((cert.clone(), keyhard));
        let keyfifo = f.path("key-fifo");
        fifo(&keyfifo);
        pairs.push((cert.clone(), keyfifo));
        pairs.push((cert.clone(), f.write("readable-key", &before.1, 0o644)));
        for (index,bytes) in [CANARY.as_bytes().to_vec(),vec![b'x';16385],before.1.repeat(2),[b"-----BEGIN ENCRYPTED PRIVATE KEY-----\nAA==\n-----END ENCRYPTED PRIVATE KEY-----\n".as_slice(),before.1.as_slice()].concat()].into_iter().enumerate(){pairs.push((cert.clone(),f.write(&format!("bad-key-{index}"),&bytes,0o600)));}
        pairs.push((cert.clone(), other.key.clone()));
        pairs.push((other.certificate.clone(), other.key.clone()));
        let runs = pairs
            .iter()
            .map(|(cert, key)| run(f, &arguments(f, &config, bind, cert, key)))
            .collect::<Vec<_>>();
        (
            runs,
            no_dial(&db),
            no_host(bind),
            good.unchanged(&before)
                && fs::metadata(&parent).unwrap().permissions().mode() & 0o7777 == 0o750,
        )
    });
    for value in &runs {
        rejected(value, 1);
    }
    assert!(no_db && no_ready && unchanged);
}
