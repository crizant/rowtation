//! Validate CSV quote structure and identify blank logical records. The csv crate
//! deliberately accepts malformed quotes and skips blank lines; this scanner
//! supplies those two policies without replacing its field decoder or copying input.
use crate::error::EngineError;

#[derive(Clone, Copy)]
enum State {
    Start,
    Unquoted,
    Quoted,
    Closed,
}

pub(super) struct Records<'a> {
    bytes: &'a [u8],
    delimiter: u8,
    offset: usize,
    line: usize,
    record: usize,
}

impl<'a> Records<'a> {
    pub(super) fn new(bytes: &'a [u8], delimiter: u8) -> Self {
        Self {
            bytes,
            delimiter,
            offset: 0,
            line: 1,
            record: 0,
        }
    }
}

/// Whether the logical record is a physical blank line, with its start line/record.
impl Iterator for Records<'_> {
    type Item = Result<(bool, usize, usize), EngineError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == self.bytes.len() {
            return None;
        }
        let start = self.offset;
        let line = self.line;
        self.record += 1;
        let mut state = State::Start;
        while self.offset < self.bytes.len() {
            let byte = self.bytes[self.offset];
            let newline = byte == b'\r' || byte == b'\n';
            if newline && !matches!(state, State::Quoted) {
                let blank = self.offset == start;
                self.offset += if byte == b'\r' && self.bytes.get(self.offset + 1) == Some(&b'\n') {
                    2
                } else {
                    1
                };
                self.line += 1;
                return Some(Ok((blank, line, self.record)));
            }
            state = match (state, byte) {
                (State::Start, b'"') => State::Quoted,
                (State::Start | State::Unquoted | State::Closed, b) if b == self.delimiter => {
                    State::Start
                }
                (State::Quoted, b'"') => State::Closed,
                (State::Closed, b'"') => State::Quoted,
                (State::Unquoted, b'"') | (State::Closed, _) => {
                    self.offset = self.bytes.len();
                    return Some(Err(EngineError::new(
                        "INVALID_CSV",
                        "Unexpected quote or characters after a closing quote.",
                    )
                    .at(self.line, self.record)));
                }
                (State::Start, _) => State::Unquoted,
                (state, _) => state,
            };
            if byte == b'\n' || (byte == b'\r' && self.bytes.get(self.offset + 1) != Some(&b'\n')) {
                self.line += 1;
            }
            self.offset += 1;
        }
        Some(if matches!(state, State::Quoted) {
            Err(EngineError::new("INVALID_CSV", "Unclosed quoted field.").at(line, self.record))
        } else {
            Ok((false, line, self.record))
        })
    }
}
