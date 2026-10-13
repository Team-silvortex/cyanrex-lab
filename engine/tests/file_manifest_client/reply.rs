use crate::*;
use axum::{
    body::{Body, Bytes},
    http::{HeaderMap, Response, StatusCode},
};
use std::{
    collections::BTreeMap,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Manifest,
    Chunk(u32),
}

#[derive(Clone)]
pub struct Piece {
    pub data: Bytes,
    pub delay: Duration,
    pub gate: Option<Arc<tokio::sync::Semaphore>>,
    pub fail: bool,
}
impl Piece {
    pub fn now(data: impl Into<Bytes>) -> Self {
        Self {
            data: data.into(),
            delay: Duration::ZERO,
            gate: None,
            fail: false,
        }
    }
    pub fn held(data: impl Into<Bytes>, gate: Arc<tokio::sync::Semaphore>) -> Self {
        Self {
            gate: Some(gate),
            ..Self::now(data)
        }
    }
    pub fn delayed(data: impl Into<Bytes>, delay: Duration) -> Self {
        Self {
            delay,
            ..Self::now(data)
        }
    }
    pub fn error() -> Self {
        Self {
            fail: true,
            ..Self::now(Bytes::new())
        }
    }
}
#[derive(Clone)]
pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub pieces: Vec<Piece>,
    pub header_gate: Option<Arc<tokio::sync::Semaphore>>,
}
impl Reply {
    pub fn good(manifest: &FileTransferManifest, kind: Kind) -> Self {
        if let Kind::Chunk(index) = kind {
            let reply = crate::legacy_fixture::Reply::good(manifest, index);
            return Self {
                status: reply.status,
                headers: reply.headers,
                pieces: vec![Piece::now(reply.pieces[0].data.clone())],
                header_gate: None,
            };
        }
        let mut reply = Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            pieces: Vec::new(),
            header_gate: None,
        };
        for (name, value) in [
            ("content-type", "application/json"),
            ("cache-control", "no-store"),
            ("pragma", "no-cache"),
            ("x-content-type-options", "nosniff"),
        ] {
            reply.headers.insert(name, value.parse().unwrap());
        }
        reply.json_bytes(serde_json::to_vec(manifest).unwrap());
        reply
    }
    pub fn json_bytes(&mut self, data: impl Into<Bytes>) {
        let data = data.into();
        self.headers
            .insert("content-length", data.len().to_string().parse().unwrap());
        self.pieces = vec![Piece::now(data)];
    }
    pub async fn response(
        self,
        kind: Kind,
        produced: Arc<Mutex<BTreeMap<Kind, usize>>>,
    ) -> Response<Body> {
        if let Some(gate) = self.header_gate {
            gate.acquire().await.unwrap().forget();
        }
        let stream = futures_util::stream::unfold(
            (self.pieces.into_iter(), produced),
            move |(mut pieces, produced)| async move {
                let piece = pieces.next()?;
                if let Some(gate) = piece.gate {
                    gate.acquire().await.unwrap().forget();
                }
                if !piece.delay.is_zero() {
                    tokio::time::sleep(piece.delay).await;
                }
                *produced.lock().unwrap().entry(kind).or_default() += 1;
                let result = if piece.fail {
                    Err(io::Error::other("controlled response frame failure"))
                } else {
                    Ok(piece.data)
                };
                Some((result, (pieces, produced)))
            },
        );
        let mut response = Response::new(Body::from_stream(stream));
        *response.status_mut() = self.status;
        *response.headers_mut() = self.headers;
        response
    }
}
