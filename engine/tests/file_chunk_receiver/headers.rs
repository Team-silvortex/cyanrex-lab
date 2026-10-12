use crate::{local::*, script::*, *};
use axum::http::{HeaderName, HeaderValue};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn receiver_requires_each_exact_singleton_header_before_polling_untrusted_body() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        for (name, _) in values(&manifest, 0) {
            for mode in 0..3 {
                let body = Script::unread();
                let calls = body.calls.clone();
                let mut r = response(&manifest, 0, body);
                match mode {
                    0 => {
                        r.headers_mut().remove(name);
                    }
                    1 => {
                        let original = r.headers()[name].clone();
                        r.headers_mut().append(name, original);
                    }
                    _ => {
                        r.headers_mut()
                            .insert(name, HeaderValue::from_static("wrong"));
                    }
                }
                rejected(
                    &f,
                    &receiver,
                    &manifest,
                    0,
                    r,
                    FileChunkReceiveError::InvalidResponse,
                )
                .await;
                assert_eq!(
                    calls.load(Ordering::SeqCst),
                    0,
                    "header {name} mode {mode} must reject before body"
                );
            }
        }
        // Canonical type/value encoding is exact, not semantic parsing of aliases.
        for (name, value) in [
            ("content-length", "017"),
            ("content-length", "17 "),
            ("content-type", "application/octet-stream; charset=utf-8"),
            (
                "content-disposition",
                "attachment; filename=\"elsewhere.bin\"",
            ),
            ("x-cyanrex-chunk-index", "00"),
            ("cache-control", "no-store, private"),
        ] {
            let body = Script::unread();
            let calls = body.calls.clone();
            let mut r = response(&manifest, 0, body);
            r.headers_mut()
                .insert(name, HeaderValue::from_str(value).unwrap());
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                r,
                FileChunkReceiveError::InvalidResponse,
            )
            .await;
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
        for (name, value) in [
            ("x-cyanrex-file-id", Uuid::new_v4().to_string()),
            ("x-cyanrex-file-version-id", Uuid::new_v4().to_string()),
            ("x-cyanrex-chunk-index", "1".to_owned()),
            ("content-length", "16".to_owned()),
            ("x-cyanrex-chunk-sha256", "1".repeat(64)),
            ("x-cyanrex-file-sha256", "2".repeat(64)),
        ] {
            let body = Script::unread();
            let calls = body.calls.clone();
            let mut r = response(&manifest, 0, body);
            r.headers_mut()
                .insert(name, HeaderValue::from_str(&value).unwrap());
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                r,
                FileChunkReceiveError::InvalidResponse,
            )
            .await;
            assert_eq!(
                calls.load(Ordering::SeqCst),
                0,
                "well-formed but wrong exact metadata {name}"
            );
        }
        healthy(&f, &receiver, 2).await;
    })
    .await;
}

#[tokio::test]
async fn receiver_rejects_non_success_forbidden_headers_and_header_inventory_overflow() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        for status in [201, 204, 206, 301, 304, 400, 404, 503] {
            let body = Script::unread();
            let calls = body.calls.clone();
            let mut r = response(&manifest, 0, body);
            *r.status_mut() = StatusCode::from_u16(status).unwrap();
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                r,
                FileChunkReceiveError::InvalidResponse,
            )
            .await;
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
        for name in [
            "content-encoding",
            "content-range",
            "transfer-encoding",
            "trailer",
            "set-cookie",
            "location",
            "etag",
            "last-modified",
            "retry-after",
        ] {
            let body = Script::unread();
            let calls = body.calls.clone();
            let mut r = response(&manifest, 0, body);
            r.headers_mut().insert(name, HeaderValue::from_static(""));
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                r,
                FileChunkReceiveError::InvalidResponse,
            )
            .await;
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        }
        let body = Script::unread();
        let calls = body.calls.clone();
        let mut r = response(&manifest, 0, body);
        for n in 0..53 {
            r.headers_mut().insert(
                HeaderName::from_bytes(format!("x-extra-{n}").as_bytes()).unwrap(),
                HeaderValue::from_static("ok"),
            );
        }
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            r,
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let mut r = response(&manifest, 0, Script::unread());
        r.headers_mut()
            .insert("x-large", HeaderValue::from_str(&"a".repeat(8192)).unwrap());
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            r,
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
        let body = Script::unread();
        let calls = body.calls.clone();
        let mut r = response(&manifest, 0, body);
        for _ in 0..53 {
            r.headers_mut()
                .append("x-repeated", HeaderValue::from_static("ok"));
        }
        assert_eq!(
            r.headers().len(),
            65,
            "duplicate unknown values each occupy an entry"
        );
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            r,
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let accepted = bytes::manifest(17, 2);
        let mut r = response(&accepted, 0, Body::from(data(&accepted, 0)));
        for n in 0..52 {
            r.headers_mut().insert(
                HeaderName::from_bytes(format!("x-extra-{n}").as_bytes()).unwrap(),
                HeaderValue::from_static("ok"),
            );
        }
        r.headers_mut()
            .insert("date", HeaderValue::from_static("ignored"));
        // Replace one optional header: exactly 64 fields, all bounded.
        r.headers_mut().remove("x-extra-51");
        assert_eq!(r.headers().len(), 64);
        receiver.receive_and_stage(&accepted, 0, r).await.unwrap();
        fixture::observe_chunk(
            &f.root.join(fixture::legacy_name(&accepted, 0)),
            &accepted,
            0,
        )
        .unwrap()
        .verified
        .unwrap();
        let exact = bytes::manifest(17, 4);
        let base_size: usize = values(&exact, 0)
            .iter()
            .map(|(name, value)| name.len() + value.len())
            .sum();
        let padding = 8192 - base_size - "x-padding".len();
        let mut r = response(&exact, 0, Body::from(data(&exact, 0)));
        r.headers_mut().insert(
            "x-padding",
            HeaderValue::from_str(&"a".repeat(padding)).unwrap(),
        );
        assert_eq!(
            r.headers()
                .iter()
                .map(|(n, v)| n.as_str().len() + v.as_bytes().len())
                .sum::<usize>(),
            8192
        );
        receiver.receive_and_stage(&exact, 0, r).await.unwrap();
        let body = Script::unread();
        let calls = body.calls.clone();
        let mut r = response(&manifest, 0, body);
        r.headers_mut().insert(
            "x-padding",
            HeaderValue::from_str(&"a".repeat(padding + 1)).unwrap(),
        );
        assert_eq!(
            r.headers()
                .iter()
                .map(|(n, v)| n.as_str().len() + v.as_bytes().len())
                .sum::<usize>(),
            8193
        );
        rejected(
            &f,
            &receiver,
            &manifest,
            0,
            r,
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        healthy(&f, &receiver, 3).await;
    })
    .await;
}
