//! Whether Pro is unlocked (docs/lm/accounts-design.md › Entitlement). There is one app; what
//! unlocks Pro is the signed-in account's entitlement, never the build. The account function's
//! `GET` answers `pro` and `plan` (the server's `profiles` row); the answer is kept with the
//! session (in the credential store), so an offline launch keeps the local Pro features unlocked
//! when the last answer said Pro. The Ask box still needs the network, and the server checks Pro
//! again on every question.
//!
//! [`access`] is the one answer to "is Pro unlocked now". A subscription's end (`expires_at`) and,
//! later, an App Store transaction feed the same [`Entitlement`] (docs/lm/app-store.md).

use serde_json::Value as Json;

/// How often a known entitlement is checked again while the app runs (seconds).
pub const REFRESH_EVERY: i64 = 6 * 3600;
/// How soon a check that failed, or hasn't answered yet, is tried again (seconds).
pub const RETRY_AFTER: i64 = 60;

/// What the server last said about the account's Pro.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entitlement {
    pub pro: bool,
    /// The plan's name (`profiles.plan`): none while Pro is free with an account.
    pub plan: Option<String>,
    /// When Pro ends (Unix seconds), for a subscription that lapses; `None`: no end.
    pub expires_at: Option<i64>,
    /// When the server said so (Unix seconds).
    pub checked_at: i64,
}

impl Entitlement {
    /// Pro, at `now`: on, and not past its end.
    pub fn active(&self, now: i64) -> bool {
        self.pro && self.expires_at.is_none_or(|end| end > now)
    }

    /// Time to ask the server again: the last answer is old, or the subscription's end has come.
    pub fn due(&self, now: i64) -> bool {
        now - self.checked_at >= REFRESH_EVERY || self.expires_at.is_some_and(|end| end <= now) || now < self.checked_at
    }

    /// The account function's `GET` answer: `{"pro": true, "plan": null, …}`. `expires_at`, when
    /// the server adds it for subscriptions, is Unix seconds.
    pub fn from_account_answer(body: &str, now: i64) -> Result<Entitlement, String> {
        let j: Json = serde_json::from_str(body).map_err(|_| "the account answer isn't readable".to_string())?;
        let pro = j.get("pro").and_then(Json::as_bool).ok_or("the account answer doesn't say whether Pro is on")?;
        let plan = j.get("plan").and_then(Json::as_str).map(str::trim).filter(|s| !s.is_empty()).map(|s| s.chars().take(64).collect());
        let expires_at = j.get("expires_at").and_then(Json::as_i64);
        Ok(Entitlement { pro, plan, expires_at, checked_at: now })
    }

    /// The entitlement an answer from the Ask service implies: an answer means Pro is on, a
    /// `not_pro` refusal that it isn't. The plan and end are kept from what was known.
    pub fn from_ask(pro: bool, known: Option<&Entitlement>, now: i64) -> Entitlement {
        let (plan, expires_at) = known.map_or((None, None), |k| (k.plan.clone(), k.expires_at));
        Entitlement { pro, plan, expires_at: if pro { expires_at.filter(|e| *e > now) } else { expires_at }, checked_at: now }
    }

    /// The fields kept in the stored session record.
    pub(crate) fn to_json(&self) -> Json {
        serde_json::json!({"pro": self.pro, "plan": self.plan, "expires_at": self.expires_at, "checked_at": self.checked_at})
    }

    pub(crate) fn from_json(j: &Json) -> Option<Entitlement> {
        Some(Entitlement {
            pro: j.get("pro")?.as_bool()?,
            plan: j.get("plan").and_then(Json::as_str).map(str::to_string),
            expires_at: j.get("expires_at").and_then(Json::as_i64),
            checked_at: j.get("checked_at").and_then(Json::as_i64).unwrap_or(0),
        })
    }
}

/// Whether Pro is unlocked, and if not, why (what a locked surface says).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The signed-in account has Pro.
    Unlocked,
    /// The developer override (`LM_DEV_PRO=1`, see [`developer_override`]); never in a release.
    Developer,
    /// Nobody is signed in: "Sign in to use this".
    SignedOut,
    /// Signed in; the server hasn't said yet whether this account has Pro.
    Checking,
    /// Signed in; the account doesn't have Pro (or it has ended).
    NotPro,
    /// This build can't sign in (no account config) and has no override: Pro features stay
    /// hidden, since nothing here could unlock them (open-source builds).
    Unavailable,
}

impl Access {
    /// Pro features work.
    pub fn unlocked(self) -> bool {
        matches!(self, Access::Unlocked | Access::Developer)
    }

    /// Pro features show, locked, with a way to unlock them.
    pub fn locked(self) -> bool {
        matches!(self, Access::SignedOut | Access::Checking | Access::NotPro)
    }

    /// Pro features show at all (working or locked).
    pub fn visible(self) -> bool {
        self != Access::Unavailable
    }
}

/// The one answer. `developer`: the override; `accounts`: this build can sign in;
/// `signed_in`: someone is; `entitlement`: what the server last said for them.
pub fn access(developer: bool, accounts: bool, signed_in: bool, entitlement: Option<&Entitlement>, now: i64) -> Access {
    if developer {
        return Access::Developer;
    }
    if !accounts {
        return Access::Unavailable;
    }
    if !signed_in {
        return Access::SignedOut;
    }
    match entitlement {
        None => Access::Checking,
        Some(e) if e.active(now) => Access::Unlocked,
        Some(_) => Access::NotPro,
    }
}

/// The developer override: `LM_DEV_PRO=1` unlocks Pro's local features without an account, for
/// development, scripted tours and screenshots. No build sets it; the Ask box still needs a route
/// (an account, or `LM_ASK_ROUTE=direct` with the developer's own key).
pub fn developer_override() -> bool {
    std::env::var("LM_DEV_PRO").is_ok_and(|v| v.trim() == "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_000_000;

    fn pro(checked_at: i64) -> Entitlement {
        Entitlement { pro: true, plan: None, expires_at: None, checked_at }
    }

    #[test]
    fn the_account_answer_reads() {
        let e = Entitlement::from_account_answer(r#"{"pro":true,"plan":null,"today":{"requests":3,"tokens":900}}"#, NOW).unwrap();
        assert_eq!(e, pro(NOW));
        let e = Entitlement::from_account_answer(r#"{"pro":false,"plan":" monthly ","expires_at":1792086400}"#, NOW).unwrap();
        assert_eq!((e.pro, e.plan.as_deref(), e.expires_at), (false, Some("monthly"), Some(1_792_086_400)));
        assert!(Entitlement::from_account_answer("{}", NOW).is_err());
        assert!(Entitlement::from_account_answer(r#"{"pro":"yes"}"#, NOW).is_err());
        assert!(Entitlement::from_account_answer("<html>", NOW).is_err());
    }

    #[test]
    fn a_subscription_ends_at_its_end() {
        let mut e = pro(NOW);
        assert!(e.active(NOW) && e.active(NOW + 10 * 365 * 86_400));
        e.expires_at = Some(NOW + 60);
        assert!(e.active(NOW + 59));
        assert!(!e.active(NOW + 60));
        assert!(!Entitlement { pro: false, ..pro(NOW) }.active(NOW));
    }

    #[test]
    fn checks_are_due_when_old_or_ended() {
        let e = pro(NOW);
        assert!(!e.due(NOW + REFRESH_EVERY - 1));
        assert!(e.due(NOW + REFRESH_EVERY));
        // A clock that went backwards: check again.
        assert!(e.due(NOW - 10));
        let ending = Entitlement { expires_at: Some(NOW + 30), ..pro(NOW) };
        assert!(!ending.due(NOW + 29) && ending.due(NOW + 30));
    }

    #[test]
    fn the_ask_services_answer_updates_it() {
        let known = Entitlement { plan: Some("yearly".into()), expires_at: Some(NOW + 100), ..pro(NOW - 50) };
        let off = Entitlement::from_ask(false, Some(&known), NOW);
        assert_eq!((off.pro, off.plan.as_deref(), off.checked_at), (false, Some("yearly"), NOW));
        assert_eq!(Entitlement::from_ask(true, None, NOW), pro(NOW));
    }

    #[test]
    fn the_stored_fields_round_trip() {
        let e = Entitlement { plan: Some("monthly".into()), expires_at: Some(NOW + 9), ..pro(NOW) };
        assert_eq!(Entitlement::from_json(&e.to_json()), Some(e));
        assert_eq!(Entitlement::from_json(&serde_json::json!({})), None);
    }

    #[test]
    fn one_answer_for_every_state() {
        use Access::*;
        let on = pro(NOW);
        let off = Entitlement { pro: false, ..pro(NOW) };
        let ended = Entitlement { expires_at: Some(NOW - 1), ..pro(NOW) };
        let cases = [
            // (developer, accounts, signed in, entitlement) → access
            (true, false, false, None, Developer),
            (true, true, true, Some(&off), Developer),
            (false, false, false, None, Unavailable),
            (false, false, true, Some(&on), Unavailable),
            (false, true, false, None, SignedOut),
            (false, true, false, Some(&on), SignedOut),
            (false, true, true, None, Checking),
            (false, true, true, Some(&on), Unlocked),
            (false, true, true, Some(&off), NotPro),
            (false, true, true, Some(&ended), NotPro),
        ];
        for (dev, accounts, signed_in, e, want) in cases {
            assert_eq!(access(dev, accounts, signed_in, e, NOW), want, "{dev} {accounts} {signed_in} {e:?}");
        }
        assert!(Unlocked.unlocked() && Developer.unlocked() && !NotPro.unlocked());
        assert!(SignedOut.locked() && Checking.locked() && NotPro.locked() && !Unlocked.locked() && !Unavailable.locked());
        assert!(!Unavailable.visible() && SignedOut.visible() && Unlocked.visible());
    }
}
