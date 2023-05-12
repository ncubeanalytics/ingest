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
