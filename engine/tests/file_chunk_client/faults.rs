use crate::{fixture::*, support::*, *};
use axum::{body::Bytes, http::StatusCode};
use std::time::Duration;

#[tokio::test]
async fn client_actual_non_success_and_wrong_exact_or_encoded_response_heads_never_save() {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = client(f, 1);
            for status in [206, 304, 401, 403, 404, 503] {
                let mut r = Reply::good(&f.manifest, 0);
                r.status = StatusCode::from_u16(status).unwrap();
                f.set_reply(r);
                denied(f, &client, &f.manifest, 0, metadata_error()).await;
            }
            for (name, value) in [
                ("x-cyanrex-file-id", Uuid::new_v4().to_string()),
                ("x-cyanrex-file-version-id", Uuid::new_v4().to_string()),
                ("x-cyanrex-chunk-index", "1".into()),
                ("x-cyanrex-chunk-sha256", "a".repeat(64)),
                ("x-cyanrex-file-sha256", "b".repeat(64)),
                ("content-encoding", "gzip".into()),
                ("content-range", "bytes 0-16/17".into()),
                ("set-cookie", format!("private={TOKEN}")),
            ] {
                let mut r = Reply::good(&f.manifest, 0);
                r.headers.insert(name, value.parse().unwrap());
                f.set_reply(r);
                denied(f, &client, &f.manifest, 0, metadata_error()).await;
            }
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client, 2).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_actual_short_and_wrong_digest_streams_fail_without_partial_output_or_sensitive_errors(
) {
    checked(Fixture::new().await, |f| {
        Box::pin(async move {
            let client = client(f, 1);
            let mut short = Reply::good(&f.manifest, 0);
            short.pieces[0].data = short.pieces[0].data.slice(..16);
            // Give the real transport an asynchronous boundary after its prefix, before the
            // intentionally premature EOF closes the connection under Content-Length:17.
            short.pieces.push(Piece {
                delay: Duration::from_millis(200),
                data: Bytes::new(),
                gate: None,
            });
            f.set_reply(short);
            denied(f, &client, &f.manifest, 0, metadata_error()).await;
            let mut wrong = Reply::good(&f.manifest, 0);
            let mut bad = wrong.pieces[0].data.to_vec();
            bad[7] ^= 1;
            wrong.pieces[0].data = Bytes::from(bad);
            f.set_reply(wrong);
            let result = client
                .fetch_and_stage(TOKEN, grant(), upload(), &f.manifest, 0)
                .await;
            let error = result.err().unwrap();
            assert_eq!(error, metadata_error());
            for text in [format!("{error}"), format!("{error:?}")] {
                assert!(!text.contains(TOKEN));
                assert!(!text.contains(&f.origin));
                assert!(!text.contains(&grant().to_string()));
            }
            assert!(bytes::tree(&f.root).is_empty());
            healthy(f, &client, 2).await;
        })
    })
    .await;
}

#[tokio::test]
async fn client_redirects_never_contact_the_target_or_forward_credentials() {
    let source = Fixture::new().await;
    let target = Fixture::new().await;
    let outcome = futures_util::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(async {
        let client = client(&source, 1);
        let mut reply = Reply::good(&source.manifest, 0);
        reply.status = StatusCode::TEMPORARY_REDIRECT;
        reply
            .headers
            .insert("location", target.origin.parse().unwrap());
        source.set_reply(reply);
        denied(&source, &client, &source.manifest, 0, metadata_error()).await;
        assert_eq!(source.observed().len(), 1);
        assert!(target.observed().is_empty());
        assert!(bytes::tree(&target.root).is_empty());
        healthy(&source, &client, 2).await;
    }))
    .await;
    source.cleanup().await;
    target.cleanup().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
