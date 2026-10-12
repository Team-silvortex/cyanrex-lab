use crate::{fixture::*, support::*, *};
use std::time::Duration;

#[tokio::test]
async fn client_origin_and_timeout_configuration_are_explicit_canonical_and_inert() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let default = FileChunkClientConfig::default();
            assert_eq!(default.transport, FileChunkTransport::Https);
            assert_eq!(default.capacity, 4);
            assert_eq!(default.header_timeout, Duration::from_secs(10));
            assert_eq!(default.idle_timeout, Duration::from_secs(5));
            assert_eq!(default.total_timeout, Duration::from_secs(60));
            assert!(default.trusted_root_pem.is_none());
            for origin in [
                "http://localhost:8000",
                "http://127.1:8000",
                "http://2130706433:8000",
                "http://0177.0.0.1:8000",
                "http://127.000.0.1:8000",
                "http://192.168.1.2:8000",
                "http://0.0.0.0:8000",
                "http://127.0.0.1:08000",
                "http://u@127.0.0.1:8000",
                "http://127.0.0.1:8000/path",
                "http://127.0.0.1:8000?x=1",
                "http://127.0.0.1:8000#x",
                " http://127.0.0.1:8000",
                "https://127.0.0.1:8000",
            ] {
                assert_eq!(
                    FileChunkClient::new_loopback(origin, f.staging.clone()).err(),
                    Some(FileChunkClientError::InvalidConfiguration),
                    "origin {origin}"
                );
            }
            for origin in [
                "http://127.0.0.1:8000",
                "http://127.99.1.2:8000",
                "http://[::1]:8000",
            ] {
                assert!(FileChunkClient::new_loopback(origin, f.staging.clone()).is_ok());
            }
            for origin in ["https://teacher.example", "https://teacher.example:8443"] {
                assert!(FileChunkClient::new_https(origin, f.staging.clone()).is_ok());
            }
            for origin in [
                f.origin.as_str(),
                "https://teacher.example/path",
                "https://u:p@teacher.example",
                "https://teacher.example?x=1",
                "https://teacher.example#fragment",
            ] {
                assert_eq!(
                    FileChunkClient::new_https(origin, f.staging.clone()).err(),
                    Some(FileChunkClientError::InvalidConfiguration)
                );
            }
            for (capacity, header, idle, total) in [
                (0, 1000, 1000, 1000),
                (17, 1000, 1000, 1000),
                (1, 999, 1000, 1000),
                (1, 60001, 1000, 1000),
                (1, 1000, 999, 1000),
                (1, 1000, 60001, 60001),
                (1, 1000, 1000, 999),
                (1, 1000, 1000, 300001),
            ] {
                let mut c = config(capacity);
                c.header_timeout = Duration::from_millis(header);
                c.idle_timeout = Duration::from_millis(idle);
                c.total_timeout = Duration::from_millis(total);
                assert_eq!(
                    FileChunkClient::with_config(&f.origin, f.staging.clone(), c).err(),
                    Some(FileChunkClientError::InvalidConfiguration)
                );
            }
            let mut c = config(16);
            c.header_timeout = Duration::from_millis(1250);
            c.idle_timeout = Duration::from_secs(60);
            c.total_timeout = Duration::from_secs(300);
            assert!(FileChunkClient::with_config(&f.origin, f.staging.clone(), c).is_ok());
            let mut c = config(1);
            c.trusted_root_pem = Some(vec![b'x'; 65537]);
            assert_eq!(
                FileChunkClient::with_config(&f.origin, f.staging.clone(), c).err(),
                Some(FileChunkClientError::InvalidConfiguration)
            );
            let mut c = FileChunkClientConfig::default();
            c.trusted_root_pem = Some(b"not a certificate".to_vec());
            assert_eq!(
                FileChunkClient::with_config("https://teacher.example", f.staging.clone(), c).err(),
                Some(FileChunkClientError::InvalidConfiguration)
            );
            assert!(f.observed().is_empty());
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client(f, 1), 2).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_invalid_token_scope_and_chunk_are_rejected_before_any_network_request() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = client(f, 1);
            for token in [
                "",
                "not-token",
                "00000000-0000-0000-0000-000000000000",
                "2BA67287-CB7C-4850-A551-966104F77B35",
                "2ba67287cb7c4850a551966104f77b35",
                "2ba67287-cb7c-4850-a551-966104f77b35\r\nCookie: other",
            ] {
                assert_eq!(
                    client
                        .fetch_and_stage(token, grant(), upload(), &f.manifest, 0)
                        .await
                        .err(),
                    Some(FileChunkClientError::InvalidInput)
                );
            }
            for field in ["workspace_id", "authority_id"] {
                let mut raw = serde_json::to_value(&f.manifest).unwrap();
                raw["workspace"][field] = serde_json::json!(Uuid::new_v4());
                let foreign: FileTransferManifest = serde_json::from_value(raw).unwrap();
                assert_eq!(
                    client
                        .fetch_and_stage(TOKEN, grant(), upload(), &foreign, 0)
                        .await
                        .err(),
                    Some(FileChunkClientError::InvalidInput)
                );
            }
            for index in [1, MAX_FILE_TRANSFER_CHUNKS as u32, u32::MAX] {
                assert_eq!(
                    client
                        .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, index)
                        .await
                        .err(),
                    Some(FileChunkClientError::InvalidInput)
                );
            }
            let empty = bytes::manifest(0, 3);
            assert_eq!(
                client
                    .fetch_and_stage(TOKEN, grant(), upload(), &empty, 0)
                    .await
                    .err(),
                Some(FileChunkClientError::InvalidInput)
            );
            assert!(f.observed().is_empty());
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client, 2).await;
        })
    })
    .await;
}
