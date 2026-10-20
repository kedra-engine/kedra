//! Scene View-specific readiness, independent of ECS handles and panel placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase {
    Unbound,
    Dependencies,
    Frame,
    Ready,
    Failed,
    Retired,
}
pub(super) struct Lifecycle {
    pub phase: Phase,
    generation: u64,
    pub diagnostic: Option<String>,
}
impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            phase: Phase::Unbound,
            generation: 0,
            diagnostic: None,
        }
    }
}
impl Lifecycle {
    pub fn begin(&mut self) -> Result<u64, String> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Scene View generation exhausted")?;
        self.phase = Phase::Dependencies;
        self.diagnostic = None;
        Ok(self.generation)
    }
    pub fn instantiated(&mut self) {
        if self.phase == Phase::Dependencies {
            self.phase = Phase::Frame;
        }
    }
    pub fn resized(&mut self) {
        if self.phase == Phase::Ready {
            self.phase = Phase::Frame;
        }
    }
    pub fn frame(&mut self) {
        if self.phase == Phase::Frame {
            self.phase = Phase::Ready;
        }
    }
    pub fn fail(&mut self, error: String) {
        self.phase = Phase::Failed;
        self.diagnostic = Some(error);
    }
    pub fn retire(&mut self) {
        self.phase = Phase::Retired;
    }
    pub fn bound(&self) -> bool {
        !matches!(self.phase, Phase::Unbound | Phase::Retired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_and_dependency_readiness_never_claim_a_frame() -> Result<(), String> {
        let mut state = Lifecycle::default();
        state.frame();
        assert_eq!(state.phase, Phase::Unbound);
        let first = state.begin()?;
        state.frame();
        assert_eq!(state.phase, Phase::Dependencies);
        state.instantiated();
        state.frame();
        assert_eq!(state.phase, Phase::Ready);
        state.resized();
        assert_eq!(state.phase, Phase::Frame);
        state.fail("dependency lost".into());
        state.frame();
        assert_eq!(state.phase, Phase::Failed);
        state.retire();
        state.frame();
        assert_eq!(state.phase, Phase::Retired);
        assert!(state.begin()? > first);
        Ok(())
    }
}
