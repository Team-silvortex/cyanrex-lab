use axum::{body::Bytes, http::HeaderMap};
use http_body::{Body, Frame, SizeHint};
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task::{Context, Poll},
};

pub enum Step {
    Data(Bytes),
    Error,
    Trailer,
    Pending,
    EmptyForever,
}
pub struct Script {
    pub steps: VecDeque<Step>,
    pub calls: Arc<AtomicUsize>,
    pub eof: Arc<AtomicUsize>,
    pub end_stream: bool,
    pub hint: u64,
}
impl Script {
    pub fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            calls: Arc::new(AtomicUsize::new(0)),
            eof: Arc::new(AtomicUsize::new(0)),
            end_stream: false,
            hint: 0,
        }
    }
    pub fn unread() -> Self {
        Self::new([Step::Pending])
    }
}
impl Body for Script {
    type Data = Bytes;
    type Error = io::Error;
    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, io::Error>>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.steps.front() {
            Some(Step::Pending) => return Poll::Pending,
            Some(Step::EmptyForever) => {
                cx.waker().wake_by_ref();
                return Poll::Ready(Some(Ok(Frame::data(Bytes::new()))));
            }
            _ => {}
        }
        Poll::Ready(match self.steps.pop_front() {
            Some(Step::Data(data)) => Some(Ok(Frame::data(data))),
            Some(Step::Error) => Some(Err(io::Error::other("synthetic body read fault"))),
            Some(Step::Trailer) => {
                let mut trailers = HeaderMap::new();
                trailers.insert("x-finish", "yes".parse().unwrap());
                Some(Ok(Frame::trailers(trailers)))
            }
            None => {
                self.eof.fetch_add(1, Ordering::SeqCst);
                None
            }
            _ => unreachable!(),
        })
    }
    fn is_end_stream(&self) -> bool {
        self.end_stream
    }
    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.hint)
    }
}
