use super::quote_windows_argument;

#[test]
fn windows_quoting_preserves_empty_spaces_quotes_and_trailing_slashes() {
    assert_eq!(quote_windows_argument("plain"), "plain");
    assert_eq!(quote_windows_argument(""), "\"\"");
    assert_eq!(quote_windows_argument("two words"), "\"two words\"");
    assert_eq!(quote_windows_argument("a\\\"b"), "\"a\\\\\\\"b\"");
    assert_eq!(
        quote_windows_argument("C:\\path with space\\"),
        "\"C:\\path with space\\\\\""
    );
}
