//! Where each region signs in: the account config (`account.json`, see
//! `packaging/lm/account.example.json`). Nothing in it is secret (the URL and the anon key are
//! public by design); it stays out of git so test projects don't land in history.

use serde_json::Value as Json;

/// Which sign-in service: international (Supabase) or mainland China (a China-hosted identity
/// service; docs/lm/accounts-design.md › Mainland China).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Region {
    International,
    China,
}

impl Region {
    /// The id used in the config and the stored session.
    pub fn id(self) -> &'static str {
        match self {
            Region::International => "international",
            Region::China => "china",
        }
    }

    pub fn from_id(id: &str) -> Option<Region> {
        match id {
            "international" => Some(Region::International),
            "china" => Some(Region::China),
            _ => None,
        }
    }

    /// What the Account pane calls it (an English catalog key: the pane translates it, `tl!`).
    pub fn label(self) -> &'static str {
        match self {
            Region::International => lm_core::tn!("International"),
            Region::China => lm_core::tn!("Mainland China"),
        }
    }

    /// The default for a system region (ISO 3166 code, `lm_ask::reference::system_region`):
    /// mainland China signs in there; everywhere else, Hong Kong, Macao and Taiwan included,
    /// internationally.
    pub fn for_system(country: Option<&str>) -> Region {
        match country {
            Some(c) if c.eq_ignore_ascii_case("CN") => Region::China,
            _ => Region::International,
        }
    }
}

/// A way to sign in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Provider {
    Apple,
    Google,
    X,
    /// A sign-in link by email.
    Email,
    WeChat,
    Alipay,
    /// A code by text message.
    Phone,
}

impl Provider {
    /// The provider for a config id. X is `x` (OAuth 2.0) or `twitter` (Supabase's legacy
    /// OAuth 1.0a provider), whichever the project turned on; the id is passed on as written.
    pub fn from_id(id: &str) -> Option<Provider> {
        Some(match id {
            "apple" => Provider::Apple,
            "google" => Provider::Google,
            "x" | "twitter" => Provider::X,
            "email" => Provider::Email,
            "wechat" => Provider::WeChat,
            "alipay" => Provider::Alipay,
            "phone" => Provider::Phone,
            _ => return None,
        })
    }

    /// The button's words (English catalog keys: the pane translates them, `tl!`).
    pub fn button(self) -> &'static str {
        match self {
            Provider::Apple => lm_core::tn!("Continue with Apple"),
            Provider::Google => lm_core::tn!("Continue with Google"),
            Provider::X => lm_core::tn!("Continue with X"),
            Provider::Email => lm_core::tn!("Email me a sign-in link"),
            Provider::WeChat => lm_core::tn!("Continue with WeChat"),
            Provider::Alipay => lm_core::tn!("Continue with Alipay"),
            Provider::Phone => lm_core::tn!("Text me a code"),
        }
    }

    /// The providers a region offers when its config doesn't list them (accounts.md › Two regions).
    pub fn defaults(region: Region) -> &'static [Provider] {
        match region {
            Region::International => &[Provider::Apple, Provider::Google, Provider::X, Provider::Email],
            Region::China => &[Provider::WeChat, Provider::Alipay, Provider::Phone, Provider::Apple],
        }
    }
}

/// One region's service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionConfig {
    /// The sign-in service's base URL (`https://<ref>.supabase.co`).
    pub auth_url: String,
    /// The public (anon) key sent as `apikey`.
    pub anon_key: String,
    /// The functions' base URL (`…/functions/v1`): `/ask` and `/account` are under it.
    pub proxy_url: String,
    /// The buttons, in order, with the id each passes to the service.
    pub providers: Vec<(Provider, String)>,
}

impl RegionConfig {
    /// The Ask proxy's address.
    pub fn ask_url(&self) -> String {
        format!("{}/ask", self.proxy_url)
    }

    /// The account function's address.
    pub fn account_url(&self) -> String {
        format!("{}/account", self.proxy_url)
    }

    /// The feedback function's address (Help › Send Feedback…, docs/lm/feedback.md).
    pub fn feedback_url(&self) -> String {
        format!("{}/feedback", self.proxy_url)
    }

    fn parse(region: Region, j: &Json) -> Result<Option<RegionConfig>, String> {
        if j.is_null() {
            return Ok(None);
        }
        let field = |k: &str| -> Result<String, String> {
            let v = j.get(k).and_then(Json::as_str).map(|s| s.trim().trim_end_matches('/').to_string()).unwrap_or_default();
            if v.is_empty() { Err(format!("{}: {k} is missing", region.id())) } else { Ok(v) }
        };
        let auth_url = field("auth_url")?;
        let anon_key = field("anon_key")?;
        let proxy_url = field("proxy_url")?;
        // A copy of the example that wasn't filled in: accounts stay off for that region.
        if [&auth_url, &anon_key, &proxy_url].iter().any(|v| is_placeholder(v)) {
            return Ok(None);
        }
        for u in [&auth_url, &proxy_url] {
            if !allowed_url(u) {
                return Err(format!("{}: {u} must be an https address", region.id()));
            }
        }
        let providers: Vec<(Provider, String)> = match j.get("providers").and_then(Json::as_array) {
            Some(list) => {
                list.iter().filter_map(Json::as_str).filter_map(|id| Provider::from_id(id).map(|p| (p, id.to_string()))).collect()
            }
            None => Provider::defaults(region).iter().map(|p| (*p, default_id(*p).to_string())).collect(),
        };
        Ok(Some(RegionConfig { auth_url, anon_key, proxy_url, providers }))
    }
}

fn default_id(p: Provider) -> &'static str {
    match p {
        Provider::Apple => "apple",
        Provider::Google => "google",
        Provider::X => "x",
        Provider::Email => "email",
        Provider::WeChat => "wechat",
        Provider::Alipay => "alipay",
        Provider::Phone => "phone",
    }
}

/// The example's placeholders (`YOUR-PROJECT-REF`, `YOUR-SUPABASE-ANON-KEY`).
pub fn is_placeholder(v: &str) -> bool {
    v.contains("YOUR-") || v.contains("PLACEHOLDER")
}

/// https, or plain http to this computer only (development and the tests' mock server).
pub fn allowed_url(u: &str) -> bool {
    u.starts_with("https://") || u.starts_with("http://127.0.0.1:") || u.starts_with("http://localhost:")
}

/// Both regions' services; a region without one has no sign-in in this build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub international: Option<RegionConfig>,
    pub china: Option<RegionConfig>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let j: Json = serde_json::from_str(text).map_err(|e| format!("account config: {e}"))?;
        Ok(Config {
            international: RegionConfig::parse(Region::International, &j["international"])?,
            china: RegionConfig::parse(Region::China, &j["china"])?,
        })
    }

    pub fn region(&self, r: Region) -> Option<&RegionConfig> {
        match r {
            Region::International => self.international.as_ref(),
            Region::China => self.china.as_ref(),
        }
    }

    /// The service feedback goes to: the international one, which posts to GitHub for everyone
    /// (mainland China included: the app reaches only this service, never GitHub).
    pub fn feedback(&self) -> Option<&RegionConfig> {
        self.international.as_ref()
    }

    /// Whether any region can sign in.
    pub fn any(&self) -> bool {
        self.international.is_some() || self.china.is_some()
    }

    /// The config for this run: `LM_ACCOUNT_CONFIG` (a path, for development), else
    /// `account.json` in the app's resources. `None` when there is neither (accounts are off and
    /// the Ask box works as before); a file that can't be read is logged and treated the same.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Option<Config> {
        let path = std::env::var_os("LM_ACCOUNT_CONFIG")
            .map(std::path::PathBuf::from)
            .or_else(|| lm_core::platform::current_resources_dir().map(|d| d.join("account.json")))?;
        let text = std::fs::read_to_string(&path).ok()?;
        match Config::parse(&text) {
            Ok(c) if c.any() => Some(c),
            Ok(_) => None,
            Err(e) => {
                log::warn!("{}: {e}", path.display());
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checked_in_example_has_only_placeholders() {
        let text = include_str!("../../../packaging/lm/account.example.json");
        let c = Config::parse(text).unwrap();
        assert_eq!(c, Config::default(), "the example must not configure any region");
    }

    #[test]
    fn a_filled_in_config_reads() {
        let c = Config::parse(
            r#"{"international": {"auth_url": "https://abc.supabase.co/", "anon_key": "k",
                "proxy_url": "https://abc.supabase.co/functions/v1", "providers": ["google", "twitter", "email", "myspace"]},
               "china": null}"#,
        )
        .unwrap();
        let i = c.region(Region::International).unwrap();
        assert_eq!(i.auth_url, "https://abc.supabase.co");
        assert_eq!(i.ask_url(), "https://abc.supabase.co/functions/v1/ask");
        assert_eq!(i.account_url(), "https://abc.supabase.co/functions/v1/account");
        assert_eq!(i.feedback_url(), "https://abc.supabase.co/functions/v1/feedback");
        assert_eq!(c.feedback(), Some(i));
        assert_eq!(
            i.providers,
            vec![(Provider::Google, "google".into()), (Provider::X, "twitter".into()), (Provider::Email, "email".into())]
        );
        assert!(c.region(Region::China).is_none());
        assert!(c.any());
    }

    #[test]
    fn missing_providers_take_the_regions_defaults() {
        let c =
            Config::parse(r#"{"china": {"auth_url": "https://id.example.cn", "anon_key": "c", "proxy_url": "https://ask.example.cn"}}"#)
                .unwrap();
        let ids: Vec<Provider> = c.china.unwrap().providers.into_iter().map(|(p, _)| p).collect();
        assert_eq!(ids, Provider::defaults(Region::China));
    }

    #[test]
    fn bad_configs_are_refused() {
        assert!(Config::parse("not json").is_err());
        assert!(Config::parse(r#"{"international": {"auth_url": "https://a", "anon_key": "k"}}"#).is_err());
        let http = r#"{"international": {"auth_url": "http://a.example", "anon_key": "k", "proxy_url": "https://a"}}"#;
        assert!(Config::parse(http).unwrap_err().contains("https"));
        // Plain http only to this computer.
        let local = r#"{"international": {"auth_url": "http://127.0.0.1:5000", "anon_key": "k", "proxy_url": "http://127.0.0.1:5000/f"}}"#;
        assert!(Config::parse(local).unwrap().international.is_some());
    }

    #[test]
    fn the_system_region_picks_the_default() {
        assert_eq!(Region::for_system(Some("CN")), Region::China);
        assert_eq!(Region::for_system(Some("cn")), Region::China);
        for c in ["HK", "MO", "TW", "US", "DE"] {
            assert_eq!(Region::for_system(Some(c)), Region::International, "{c}");
        }
        assert_eq!(Region::for_system(None), Region::International);
        for r in [Region::International, Region::China] {
            assert_eq!(Region::from_id(r.id()), Some(r));
        }
    }
}
