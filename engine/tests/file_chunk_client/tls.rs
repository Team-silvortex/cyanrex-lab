use crate::{fixture::*, tls_fixture::*, *};
use cyanrex_engine::services::file_chunk_client::{FileChunkClientConfig, FileChunkClientError};
use futures_util::FutureExt;
use std::{fs, net::Ipv4Addr, panic::AssertUnwindSafe};

#[tokio::test]
#[ignore = "requires explicit native OpenSSL loopback TLS fixture"]
async fn openssl_client_requires_trusted_ca_and_exact_ip_san_before_real_private_staging() {
    let f = Fixture::new().await;
    let result = AssertUnwindSafe(async {
        let certificates = Certificates::generate(&f);
        let unknown = TlsServer::start(&certificates, &f, Ipv4Addr::LOCALHOST);
        let client = FileChunkClient::new_https(&unknown.origin, f.staging.clone()).unwrap();
        assert_eq!(
            client
                .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0)
                .await
                .err(),
            Some(FileChunkClientError::Unavailable),
            "private test CA is not a system-trusted issuer"
        );
        let unknown = unknown.finish();
        assert_eq!(
            unknown.received_bytes, 0,
            "untrusted TLS must receive no decrypted HTTP bytes"
        );
        assert!(!unknown.responded);
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);

        let mismatch = TlsServer::start(&certificates, &f, Ipv4Addr::new(127, 0, 0, 2));
        let configured = FileChunkClientConfig {
            trusted_root_pem: Some(certificates.ca.clone()),
            ..FileChunkClientConfig::default()
        };
        let client =
            FileChunkClient::with_config(&mismatch.origin, f.staging.clone(), configured.clone())
                .unwrap();
        assert_eq!(
            client
                .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0)
                .await
                .err(),
            Some(FileChunkClientError::Unavailable),
            "trusting a CA does not bypass the IP SAN check"
        );
        let mismatch = mismatch.finish();
        assert_eq!(
            mismatch.received_bytes, 0,
            "wrong SAN must receive no decrypted HTTP bytes"
        );
        assert!(!mismatch.responded);
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);

        let trusted = TlsServer::start(&certificates, &f, Ipv4Addr::LOCALHOST);
        let client =
            FileChunkClient::with_config(&trusted.origin, f.staging.clone(), configured).unwrap();
        let ack = client
            .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0)
            .await
            .unwrap();
        assert_eq!(ack.file(), &FileVersionRef::from_manifest(&f.manifest));
        assert_eq!(ack.byte_length(), 17);
        let trusted = trusted.finish();
        assert!(trusted.received_bytes > 0);
        assert!(
            trusted.responded,
            "correct TLS peer must receive and answer the real HTTP request"
        );
        let observed = saved(&f.root.join(name(&f.manifest)), &f.manifest).unwrap();
        observed.verified.unwrap();
        assert_eq!(observed.bytes, (0..17).map(bytes::byte).collect::<Vec<_>>());
        assert_eq!(observed.mode & 0o777, 0o400);
        assert_eq!(observed.links, 1);
        assert!(observed.same_inode);
    })
    .catch_unwind()
    .await;
    f.cleanup().await;
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}
