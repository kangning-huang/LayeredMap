//! Which way the Ask box goes (docs/lm/accounts-design.md › The Ask box: which route): straight
//! to OpenRouter with a developer's key, or through the Ask proxy with the account's token.

/// The Ask box's route for one request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskRoute {
    /// OpenRouter with a developer's key on this computer (development and evaluation only).
    Direct,
    /// The Ask proxy, with the signed-in account's access token.
    Proxy,
    /// Neither: "the assistant isn't set up in this build".
    Unavailable,
}

impl AskRoute {
    /// `env` is `LM_ASK_ROUTE` (`direct` or `proxy`); `has_key`: a developer's key is
    /// present; `proxy`: the account config has a proxy for the region.
    pub fn choose(env: Option<&str>, has_key: bool, proxy: bool) -> AskRoute {
        match env.map(str::trim) {
            Some(v) if v.eq_ignore_ascii_case("direct") && has_key => return AskRoute::Direct,
            Some(v) if v.eq_ignore_ascii_case("proxy") && proxy => return AskRoute::Proxy,
            _ => {}
        }
        if proxy {
            AskRoute::Proxy
        } else if has_key {
            // A developer's machine with no account config (`LM_DEV_PRO=1` unlocks the Ask box):
            // the developer's own key. No build carries a key.
            AskRoute::Direct
        } else {
            AskRoute::Unavailable
        }
    }

    /// [`AskRoute::choose`] with `LM_ASK_ROUTE` from the environment.
    pub fn from_env(has_key: bool, proxy: bool) -> AskRoute {
        AskRoute::choose(std::env::var("LM_ASK_ROUTE").ok().as_deref(), has_key, proxy)
    }
}

#[cfg(test)]
mod tests {
    use super::AskRoute::{self, *};

    #[test]
    fn the_route_follows_the_config_the_key_and_the_switch() {
        let cases = [
            // (LM_ASK_ROUTE, key, proxy configured) → route
            (None, false, false, Unavailable),
            (None, true, false, Direct),
            (None, false, true, Proxy),
            (None, true, true, Proxy),
            (Some("direct"), true, true, Direct),
            (Some(" Direct "), true, false, Direct),
            // The switch can't conjure what isn't there.
            (Some("direct"), false, true, Proxy),
            (Some("direct"), false, false, Unavailable),
            (Some("proxy"), true, false, Direct),
            (Some("proxy"), true, true, Proxy),
            (Some("something"), true, false, Direct),
        ];
        for (env, key, proxy, want) in cases {
            assert_eq!(AskRoute::choose(env, key, proxy), want, "{env:?} key={key} proxy={proxy}");
        }
    }
}
