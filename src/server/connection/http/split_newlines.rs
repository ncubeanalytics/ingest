use bytes::{Buf, Bytes};

const NEWLINE: u8 = 0x0A;
const CARRIAGE_RETURN: u8 = 0x0D;

pub fn split_newlines(mut data: Bytes) -> Vec<Bytes> {
    let mut parts = Vec::with_capacity(100);

    loop {
        if let Some(newline_pos) = data.iter().position(|c| *c == NEWLINE) {
            if newline_pos == 0 {
                // this check and the continue guarantees that !left_part.is_empty() later
                data.advance(1);
                continue;
            }
            if newline_pos == 1 && data[0] == CARRIAGE_RETURN {
                data.advance(2);
                continue;
            }
            let mut left_part = data.split_to(newline_pos);
            data.advance(1); // data now starts with the newline, so move 1 further

            // if the last byte is a \r we got here from a \r\n, so right-trim the \r
            if left_part[left_part.len() - 1] == CARRIAGE_RETURN {
                left_part.truncate(left_part.len() - 1);
            }

            // left-trim \n and \r\n
            loop {
                if left_part[0] == NEWLINE {
                    left_part.advance(1);
                } else if left_part.len() >= 2
                    && left_part[0] == CARRIAGE_RETURN
                    && left_part[1] == NEWLINE
                {
                    left_part.advance(2)
                } else {
                    break;
                }
            }

            if !left_part.is_empty() {
                parts.push(left_part);
            }
        } else {
            break;
        }
    }

    if !data.is_empty() {
        parts.push(data);
    }
    parts
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn no_newline() {
        let split = split_newlines("ab\rc".into());

        assert_eq!(split, vec!["ab\rc"]);
    }

    #[test]
    fn nls() {
        let split = split_newlines("a\nb\nc".into());

        assert_eq!(split, vec!["a", "b", "c"]);
    }

    #[test]
    fn crnls() {
        let split = split_newlines("a\r\nb\r\nc".into());

        assert_eq!(split, vec!["a", "b", "c"]);
    }

    #[test]
    fn several_nls() {
        let split = split_newlines("a\n\n\nb\n\n\nc".into());

        assert_eq!(split, vec!["a", "b", "c"]);
    }

    #[test]
    fn several_cr_nls() {
        let split = split_newlines("a\r\n\r\n\nb\r\n\r\n\r\nc".into());

        assert_eq!(split, vec!["a", "b", "c"]);
    }

    #[test]
    fn trailing_newline() {
        let split = split_newlines("abc\n".into());

        assert_eq!(split, vec!["abc"]);
    }

    #[test]
    fn trailing_crnl() {
        let split = split_newlines("abc\r\n".into());

        assert_eq!(split, vec!["abc"]);
    }

    #[test]
    fn trailing_newlines() {
        let split = split_newlines("abc\n\n".into());

        assert_eq!(split, vec!["abc"]);
    }

    #[test]
    fn trailing_crnls() {
        let split = split_newlines("abc\r\n\r\n".into());

        assert_eq!(split, vec!["abc"]);
    }

    #[test]
    fn extra_carriage_return() {
        let split = split_newlines("a\r\r\nb\r\r\nc\r\r\n".into());

        assert_eq!(split, vec!["a\r", "b\r", "c\r"]);
    }

    #[test]
    fn empty() {
        let split = split_newlines("".into());

        assert_eq!(split.len(), 0);
    }

    #[test]
    fn only_newlines() {
        let split = split_newlines("\n\n".into());

        assert_eq!(split.len(), 0);
    }
}
