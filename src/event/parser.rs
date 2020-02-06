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
            return Err(json::Error::wrong_type("array").into());
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

#[cfg(test)]
mod tests {
    use json::Error as JSONError;

    use super::*;
    use crate::error::Error;

    #[test]
    fn parser_accepts_only_valid_json() {
        let buf = Bytes::from("this is not valid JSON");

        let res = EventObjParser::new(buf);

        assert!(res.is_err());

        match res.unwrap_err() {
            Error::JSON(JSONError::UnexpectedCharacter { .. }) => {}
            _ => panic!("Expected invalid JSON error"),
        }
    }

    #[test]
    fn parser_accepts_only_json_array() {
        let cases = vec![
            "null",
            "true",
            "32",
            "32.1",
            r#""test""#,
            r#"{"object": 1}"#,
        ];

        for case in cases {
            let buf = Bytes::from(case);

            let res = EventObjParser::new(buf);

            assert!(res.is_err());

            match res.unwrap_err() {
                Error::JSON(JSONError::WrongType(t)) if t == "array" => {}
                _ => panic!("Expected wrong JSON type error"),
            }
        }
    }

    #[test]
    fn parser_returns_objects() -> Result<()> {
        let obj_1 = r#"{"simple": "object"}"#;

        let obj_2 = r#"{
            "object": "with",
            "multiple": "fields",
            "is for testing": true,
            "int": 1,
            "float": 1.2
        }"#;

        let obj_3 = r#"{
            "object": "with nested objects",
            "nest": {
                "another": "nested object",
                "here": {
                    "final": "object"
                }
            },
            "and": "an array",
            "an array": ["some", true, "json values", 43],
            "plus": "an object array",
            "objects": [
                {
                    "this": "object",
                    "simple": true
                },
                {
                    "this": "nested object",
                    "simple": false,
                    "because": {
                        "more": {
                            "objects": "to parse",
                            "value": 5
                        }
                    }
                }
            ]
        }"#;

        let objects = Bytes::from(format!("[{},{},{}]", obj_1, obj_2, obj_3));

        let mut events = EventObjParser::new(objects)?;

        for obj in &[obj_1, obj_2, obj_3] {
            assert_eq!(events.next(), Some(Bytes::from(*obj)));
        }

        assert_eq!(events.next(), None);

        Ok(())
    }
}
