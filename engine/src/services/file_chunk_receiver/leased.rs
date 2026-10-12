use std::io::{self, Cursor, Read};
use tokio::sync::OwnedSemaphorePermit;

pub(super) struct LeasedCursor {
    cursor: Cursor<Vec<u8>>,
    #[cfg(test)]
    observer: Option<Box<dyn FnOnce() + Send + 'static>>,
    // Drop the private bytes before making receive/buffer capacity available again.
    _permit: OwnedSemaphorePermit,
    // The optional outer request budget drops last, after the bytes and receiver lease.
    _caller: Option<OwnedSemaphorePermit>,
}

impl LeasedCursor {
    pub(super) fn new(bytes: Vec<u8>, permit: OwnedSemaphorePermit) -> Self {
        Self {
            cursor: Cursor::new(bytes),
            #[cfg(test)]
            observer: None,
            _permit: permit,
            _caller: None,
        }
    }

    pub(super) fn retain_caller(&mut self, permit: Option<OwnedSemaphorePermit>) {
        self._caller = permit;
    }

    #[cfg(test)]
    pub(super) fn observe_first_read(&mut self, observer: impl FnOnce() + Send + 'static) {
        self.observer = Some(Box::new(observer));
    }
}

impl Read for LeasedCursor {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        #[cfg(test)]
        if let Some(observer) = self.observer.take() {
            observer();
        }
        self.cursor.read(buffer)
    }
}
