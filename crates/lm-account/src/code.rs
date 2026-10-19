//! The 6-digit code in the sign-in email (docs/lm/accounts-design.md › Email: the link or the
//! code). The email carries both the link and the code; the code can be typed into the app, so an
//! email read on a phone still signs in on this computer.

use serde_json::json;

/// How many digits the code has (the project's `otp_length`, services/ask-proxy/supabase/config.toml).
pub const LEN: usize = 6;

/// What the person typed or pasted, as a code: spaces and dashes dropped ("123 456", "123-456"),
/// then exactly six digits. `None` for anything else.
pub fn normalize(typed: &str) -> Option<String> {
    let code: String = typed.chars().filter(|c| !c.is_whitespace() && !matches!(c, '-' | '‐' | '‑' | '–')).collect();
    (code.len() == LEN && code.bytes().all(|b| b.is_ascii_digit())).then_some(code)
}

/// What the code field keeps of an edit or a paste: the digits, at most six.
pub fn field_text(typed: &str) -> String {
    typed.chars().filter(char::is_ascii_digit).take(LEN).collect()
}

/// The body of `POST /auth/v1/verify` for an email code.
pub fn verify_body(email: &str, code: &str) -> serde_json::Value {
    json!({"type": "email", "email": email, "token": code})
}

/// Supabase's verify address.
pub fn verify_url(auth_url: &str) -> String {
    format!("{auth_url}/auth/v1/verify")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spaces_and_dashes_are_dropped_and_six_digits_are_required() {
        for ok in ["123456", "123 456", " 123 456 ", "123-456", "12 34 56", "123\u{a0}456", "123–456", "1 2 3 4 5 6"] {
            assert_eq!(normalize(ok).as_deref(), Some("123456"), "{ok:?}");
        }
        for bad in ["", "12345", "1234567", "12345a", "abcdef", "123.456", "123_456", "١٢٣٤٥٦", "123 45"] {
            assert_eq!(normalize(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn the_field_keeps_digits_only() {
        assert_eq!(field_text("123 456"), "123456");
        assert_eq!(field_text("123-456"), "123456");
        assert_eq!(field_text("Your code: 654321."), "654321");
        assert_eq!(field_text("1234567"), "123456");
        assert_eq!(field_text("12a"), "12");
        assert_eq!(field_text(""), "");
    }

    #[test]
    fn the_request_body_is_supabases_email_verify() {
        let b = verify_body("kim@example.com", "123456");
        assert_eq!(b, json!({"type": "email", "email": "kim@example.com", "token": "123456"}));
        assert_eq!(b.as_object().map(|o| o.len()), Some(3), "nothing else is sent");
        assert_eq!(verify_url("https://abc.supabase.co"), "https://abc.supabase.co/auth/v1/verify");
    }
}
