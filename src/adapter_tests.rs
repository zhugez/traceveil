use super::*;
use std::io::{Cursor, Read};
struct ChangingReader {
    inner: Cursor<Vec<u8>>,
    replacement: Option<Vec<u8>>,
}
impl Read for ChangingReader {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(bytes)
    }
}
impl Seek for ChangingReader {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if let Some(replacement) = self.replacement.take() {
            self.inner = Cursor::new(replacement);
        }
        self.inner.seek(position)
    }
}
#[test]
fn mutation_between_pi_passes_never_returns_a_successful_report() {
    let bytes = include_bytes!("../benchmarks/fixtures/demo.pi.jsonl").to_vec();
    let replacement = String::from_utf8(bytes.clone())
        .unwrap()
        .replace("FAILED", "changed")
        .into_bytes();
    let source = ChangingReader {
        inner: Cursor::new(bytes),
        replacement: Some(replacement),
    };
    assert_eq!(
        pi_reader(source, None, Settings::default())
            .unwrap_err()
            .code,
        "SOURCE_CHANGED"
    );
}
#[test]
fn raw_record_limit_applies_even_when_no_events_are_emitted() {
    let source = vec![b'\n'; MAX_RECORDS + 1];
    assert_eq!(
        scan(Cursor::new(source), |_, _| Ok(())).err().unwrap().code,
        "INPUT_TOO_LARGE"
    );
}
