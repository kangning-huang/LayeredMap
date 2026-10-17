//! Phase feature flags (WP1.1, Gate protocol › What makes rollback cheap).
//!
//! Every surface and action declares the phase that introduces it (`min_phase`). Launching with
//! `LM_PHASE=N` hides everything from later phases, so a broken surface can be switched off
//! without reverting code. The environment is read by the app; this module only interprets it.

/// The phase this build implements: surfaces from later phases are hidden whatever `LM_PHASE` says.
pub const BUILD_PHASE: u8 = 3;

/// Which phases' surfaces are shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flags {
    phase: u8,
}

impl Flags {
    /// The flags for an `LM_PHASE` value (`None` when unset). A value that isn't a small number,
    /// or is above [`BUILD_PHASE`], falls back to [`BUILD_PHASE`].
    pub fn from_env(lm_phase: Option<&str>) -> Flags {
        let phase = lm_phase.and_then(|s| s.trim().parse::<u8>().ok()).filter(|p| *p <= BUILD_PHASE).unwrap_or(BUILD_PHASE);
        Flags { phase }
    }

    /// Flags showing everything up to `phase` (tests and screenshots).
    pub fn up_to(phase: u8) -> Flags {
        Flags { phase: phase.min(BUILD_PHASE) }
    }

    pub fn phase(self) -> u8 {
        self.phase
    }

    /// Whether a surface introduced in `min_phase` is shown.
    pub fn shows(self, min_phase: u8) -> bool {
        min_phase <= self.phase
    }
}

impl Default for Flags {
    fn default() -> Self {
        Flags { phase: BUILD_PHASE }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lm_phase_hides_later_surfaces() {
        let f = Flags::from_env(Some("0"));
        assert!(f.shows(0));
        assert!(!f.shows(1));
        assert!(Flags::from_env(None).shows(BUILD_PHASE));
    }

    #[test]
    fn junk_or_future_phases_fall_back_to_the_build() {
        assert_eq!(Flags::from_env(Some("x")).phase(), BUILD_PHASE);
        assert_eq!(Flags::from_env(Some("9")).phase(), BUILD_PHASE);
        assert_eq!(Flags::from_env(Some(" 0 ")).phase(), 0);
        // A surface from a phase not built yet is never shown.
        assert!(!Flags::default().shows(BUILD_PHASE + 1));
    }
}
