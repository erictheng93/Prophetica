use sdk::masking::mask_email;

#[test]
fn email_with_at_keeps_first_char_and_domain() {
    assert_eq!(mask_email("member-001@example.com"), "m***@example.com");
}

#[test]
fn short_value_without_at_returns_four_stars() {
    assert_eq!(mask_email("abc"), "****");
}

#[test]
fn longer_value_without_at_returns_four_stars() {
    assert_eq!(mask_email("not-an-email"), "****");
}

#[test]
fn empty_string_returns_four_stars() {
    assert_eq!(mask_email(""), "****");
}

#[test]
fn email_starting_with_at_returns_four_stars() {
    assert_eq!(mask_email("@example.com"), "****");
}
