use bytes::Bytes;

use crate::error::Result;

/// Parser for JSON object events.
/// Initialize with a JSON byte buffer.
/// Can be used to iterate over the raw JSON objects (as bytes) found in the
/// given buf.
#[derive(Debug, Default)]
pub struct EventObjParser {
    buf: Bytes,
    cursor: usize,
}

impl EventObjParser {
    pub fn new(buf: Bytes) -> Result<Self> {
        let json_data = json::parse(std::str::from_utf8(buf.as_ref())?)?;
        if !json_data.is_array() {
            Err(json::Error::wrong_type("array"))?;
        }

        Ok(Self { buf, cursor: 0 })
    }
}

impl Iterator for EventObjParser {
    type Item = Bytes;

    fn next(&mut self) -> Option<Bytes> {
        let mut obj_start = self.cursor;
        let mut braces = 0;

        for i in self.cursor..self.buf.len() {
            match self.buf.get(i) {
                None => return None,

                Some(b'{') => {
                    if braces == 0 {
                        obj_start = i;
                    }

                    braces += 1;
                }

                Some(b'}') => {
                    braces -= 1;

                    if braces == 0 {
                        // there cannot be another valid JSON
                        // object starting sooner than i+2:
                        // | i |i+1|i+2|
                        // | } | , | { |
                        self.cursor = i + 2;

                        return Some(self.buf.slice(obj_start..=i));
                    }
                }

                // do nothing for all other chars
                _ => {}
            }
        }

        None
    }
}
