use super::write_pretty;
use serde::{ser::SerializeSeq, Serialize};
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

struct ObservedRows<'a> {
    visited: Arc<AtomicUsize>,
    value: &'a str,
    count: usize,
}

impl Serialize for ObservedRows<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.count))?;
        for _ in 0..self.count {
            self.visited.fetch_add(1, Ordering::Relaxed);
            seq.serialize_element(self.value)?;
        }
        seq.end()
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    visited: Arc<AtomicUsize>,
    first_write_visited: Option<usize>,
    calls: usize,
    largest_write: usize,
    max_chunk: Option<usize>,
    fail_after: Option<usize>,
    fail_flush: bool,
    interrupt_once: bool,
    flushes: usize,
}

impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.first_write_visited
            .get_or_insert_with(|| self.visited.load(Ordering::Relaxed));
        self.calls += 1;
        self.largest_write = self.largest_write.max(bytes.len());
        if self.interrupt_once {
            self.interrupt_once = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self
            .fail_after
            .is_some_and(|limit| self.bytes.len() >= limit)
        {
            return Err(io::Error::other("injected write failure"));
        }
        let length = bytes.len().min(self.max_chunk.unwrap_or(usize::MAX)).min(
            self.fail_after
                .map_or(usize::MAX, |limit| limit - self.bytes.len()),
        );
        self.bytes.extend_from_slice(&bytes[..length]);
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn pretty_json_is_written_before_all_rows_are_serialized() {
    let value = "引号\"、反斜线\\、换行\n\t\u{0}".repeat(64);
    let visited = Arc::new(AtomicUsize::new(0));
    let rows = ObservedRows {
        visited: visited.clone(),
        value: &value,
        count: 256,
    };
    let mut writer = Writer {
        visited,
        ..Writer::default()
    };
    write_pretty(&mut writer, &rows).unwrap();
    assert!(
        writer.first_write_visited.unwrap() < rows.count,
        "must stream before encoding all rows"
    );
    assert_eq!(
        writer.bytes,
        serde_json::to_vec_pretty(&vec![value.as_str(); rows.count]).unwrap()
    );
    assert!(writer.calls > 1);
    assert!(
        writer.largest_write <= 64 * 1024,
        "small records must use bounded buffering"
    );
    assert_eq!(writer.flushes, 1, "explicitly flush before committing");
}

#[test]
fn short_and_interrupted_writes_preserve_large_source_and_empty_arrays() {
    for value in [
        serde_json::json!([]),
        serde_json::json!({"source": "中文\n\\\"".repeat(20000)}),
    ] {
        let mut writer = Writer {
            max_chunk: Some(17),
            interrupt_once: true,
            ..Writer::default()
        };
        write_pretty(&mut writer, &value).unwrap();
        assert_eq!(writer.bytes, serde_json::to_vec_pretty(&value).unwrap());
        assert_eq!(writer.flushes, 1);
    }
}

#[test]
fn partial_write_and_final_flush_failures_are_returned() {
    let value = vec!["x".repeat(1024); 128];
    for limit in [0, 17, 64 * 1024 + 17] {
        let mut writer = Writer {
            fail_after: Some(limit),
            ..Writer::default()
        };
        assert!(write_pretty(&mut writer, &value)
            .unwrap_err()
            .contains("injected write failure"));
        assert_eq!(writer.bytes.len(), limit);
    }
    let mut writer = Writer {
        fail_flush: true,
        ..Writer::default()
    };
    assert!(write_pretty(&mut writer, &value)
        .unwrap_err()
        .contains("injected flush failure"));
    assert_eq!(writer.flushes, 1);
}

#[test]
fn serializer_errors_are_not_successful_writes() {
    struct Rejected;
    impl Serialize for Rejected {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("injected encode failure"))
        }
    }
    assert!(write_pretty(io::sink(), &Rejected)
        .unwrap_err()
        .contains("injected encode failure"));
}
