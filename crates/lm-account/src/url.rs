//! The addresses of a sign-in: the authorize URL the browser opens, the loopback redirect, and
//! percent-encoding for both.

/// Percent-encodes everything but RFC 3986's unreserved characters.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Decodes a query value (`%XX` and `+` for a space). `None` for a broken escape or bytes that
/// aren't UTF-8.
pub fn decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'%' => {
                let hex = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// A query string's pairs, decoded. `None` if any part can't be decoded.
pub fn query_pairs(query: &str) -> Option<Vec<(String, String)>> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            Some((decode(k)?, decode(v)?))
        })
        .collect()
}

/// The loopback redirect for a port and state (docs/lm/accounts-design.md › The redirect).
pub fn redirect_uri(port: u16, state: &str) -> String {
    format!("http://127.0.0.1:{port}/callback?state={}", encode(state))
}

/// Supabase's authorize URL for a provider, with the PKCE challenge (S256).
pub fn authorize_url(auth_url: &str, provider_id: &str, redirect: &str, challenge: &str) -> String {
    format!(
        "{auth_url}/auth/v1/authorize?provider={}&redirect_to={}&code_challenge={}&code_challenge_method=s256",
        encode(provider_id),
        encode(redirect),
        encode(challenge)
    )
}

/// Supabase's email-link request address: the link in the email comes back to `redirect`.
pub fn otp_url(auth_url: &str, redirect: &str) -> String {
    format!("{auth_url}/auth/v1/otp?redirect_to={}", encode(redirect))
}

/// Supabase's password grant (temporary App Review path; feature `review-password-signin`).
pub fn password_token_url(auth_url: &str) -> String {
    format!("{auth_url}/auth/v1/token?grant_type=password")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trips() {
        let s = "a b&c=d/é?#~._-";
        assert_eq!(encode(s), "a%20b%26c%3Dd%2F%C3%A9%3F%23~._-");
        assert_eq!(decode(&encode(s)).as_deref(), Some(s));
        assert_eq!(decode("a+b%2Bc").as_deref(), Some("a b+c"));
        assert_eq!(decode("%zz"), None);
        assert_eq!(decode("%4"), None);
        assert_eq!(decode("%FF"), None);
    }

    #[test]
    fn the_authorize_url_carries_provider_redirect_and_challenge() {
        let redirect = redirect_uri(54321, "st/ate");
        assert_eq!(redirect, "http://127.0.0.1:54321/callback?state=st%2Fate");
        let u = authorize_url("https://abc.supabase.co", "google", &redirect, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(
            u,
            "https://abc.supabase.co/auth/v1/authorize?provider=google\
             &redirect_to=http%3A%2F%2F127.0.0.1%3A54321%2Fcallback%3Fstate%3Dst%252Fate\
             &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=s256"
        );
        // What the service reads back out of it.
        let (_, q) = u.split_once('?').unwrap();
        let pairs = query_pairs(q).unwrap();
        assert!(pairs.contains(&("redirect_to".into(), redirect.clone())));
        assert!(pairs.contains(&("code_challenge_method".into(), "s256".into())));
        assert_eq!(
            otp_url("https://abc.supabase.co", &redirect),
            format!("https://abc.supabase.co/auth/v1/otp?redirect_to={}", encode(&redirect))
        );
        assert_eq!(password_token_url("https://abc.supabase.co"), "https://abc.supabase.co/auth/v1/token?grant_type=password");
    }

    #[test]
    fn query_pairs_decode_and_refuse_broken_escapes() {
        assert_eq!(
            query_pairs("a=1&b=&c&&d=x%20y").unwrap(),
            vec![("a".into(), "1".into()), ("b".into(), "".into()), ("c".into(), "".into()), ("d".into(), "x y".into())]
        );
        assert!(query_pairs("a=%").is_none());
    }
}
