use crate::{local::*, script::*, *};
use axum::body::Bytes;
use std::{
    marker::PhantomPinned,
    pin::Pin,
    sync::{atomic::Ordering, Mutex},
    task::{Context, Poll},
};

#[tokio::test]
async fn receiver_rejects_short_long_wrong_digest_error_and_trailer_before_creating_files() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        let good = data(&manifest, 0);
        let mut bad = good.to_vec();
        bad[8] ^= 1;
        for steps in [
            vec![],
            vec![Step::Data(good.slice(..16))],
            vec![
                Step::Data(good.clone()),
                Step::Data(Bytes::from_static(b"x")),
            ],
            vec![Step::Data(Bytes::from(vec![0; 4 * 1024 * 1024 + 1]))],
            vec![Step::Data(Bytes::from(bad))],
            vec![Step::Error],
            vec![Step::Data(good.clone()), Step::Error],
            vec![Step::Data(good.clone()), Step::Trailer],
            vec![Step::Trailer],
        ] {
            rejected(
                &f,
                &receiver,
                &manifest,
                0,
                response(&manifest, 0, Script::new(steps)),
                FileChunkReceiveError::InvalidResponse,
            )
            .await;
        }
        healthy(&f, &receiver, 2).await;
    })
    .await;
}

struct PinnedBody {
    inner: Mutex<Script>,
    _pin: PhantomPinned,
}
impl http_body::Body for PinnedBody {
    type Data = Bytes;
    type Error = std::io::Error;
    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        http_body::Body::poll_frame(
            Pin::new(&mut *self.as_ref().get_ref().inner.lock().unwrap()),
            cx,
        )
    }
    fn is_end_stream(&self) -> bool {
        true
    }
    fn size_hint(&self) -> http_body::SizeHint {
        http_body::SizeHint::with_exact(u64::MAX)
    }
}

#[tokio::test]
async fn receiver_uses_explicit_eof_not_untrusted_hints_and_accepts_fragmented_non_unpin_body() {
    checked(|f| async move {
        let receiver = f.receiver(1);
        let manifest = bytes::manifest(17, 1);
        let bytes = data(&manifest, 0);
        let mut body = Script::new([
            Step::Data(Bytes::new()),
            Step::Data(bytes.slice(..5)),
            Step::Data(Bytes::new()),
            Step::Data(bytes.slice(5..)),
        ]);
        body.end_stream = true;
        body.hint = u64::MAX;
        let eof = body.eof.clone();
        let pinned = PinnedBody {
            inner: Mutex::new(body),
            _pin: PhantomPinned,
        };
        receiver
            .receive_and_stage(&manifest, 0, response(&manifest, 0, pinned))
            .await
            .unwrap();
        assert_eq!(
            eof.load(Ordering::SeqCst),
            1,
            "exact None EOF must be observed despite is_end_stream"
        );
        let observed = fixture::observe_chunk(
            &f.root.join(fixture::legacy_name(&manifest, 0)),
            &manifest,
            0,
        )
        .unwrap();
        observed.verified.unwrap();
        assert_eq!(observed.bytes, bytes);
        let next = bytes::manifest(17, 2);
        let mut body = Script::new([
            Step::Data(data(&next, 0)),
            Step::Data(Bytes::from_static(b"!")),
        ]);
        body.end_stream = true;
        body.hint = 17;
        rejected(
            &f,
            &receiver,
            &next,
            0,
            response(&next, 0, body),
            FileChunkReceiveError::InvalidResponse,
        )
        .await;
        healthy(&f, &receiver, 3).await;
    })
    .await;
}
