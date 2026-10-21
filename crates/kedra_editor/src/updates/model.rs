//! Operation authority independent of header entities, workers and filesystem state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Phase {
    #[default]
    Hidden,
    Available,
    Downloading,
    Verifying,
    Ready,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Effect {
    Download(u64),
    Apply(u64),
}

#[derive(Default)]
pub(super) struct Model {
    pub phase: Phase,
    pub generation: u64,
    pub error: String,
    pub applying: bool,
}

impl Model {
    pub fn invalidated(&mut self, generation: u64, error: String) {
        if generation == self.generation && self.applying {
            self.phase = Phase::Error;
            self.applying = false;
            self.error = error;
        }
    }
    pub fn applied(&mut self, generation: u64, error: String) {
        if generation == self.generation && self.applying {
            self.applying = false;
            self.error = error;
        }
    }
    pub fn available(&mut self) {
        self.phase = Phase::Available;
    }
    pub fn activate(&mut self) -> Option<Effect> {
        match self.phase {
            Phase::Available | Phase::Error => {
                self.generation = self.generation.checked_add(1)?;
                self.phase = Phase::Downloading;
                self.error.clear();
                Some(Effect::Download(self.generation))
            }
            Phase::Ready if !self.applying => {
                self.applying = true;
                Some(Effect::Apply(self.generation))
            }
            _ => None,
        }
    }
    pub fn receive(&mut self, generation: u64, phase: Phase, error: String) {
        if generation != self.generation {
            return;
        }
        if matches!(
            (self.phase, phase),
            (Phase::Downloading, Phase::Verifying | Phase::Error)
                | (Phase::Verifying, Phase::Ready | Phase::Error)
        ) {
            self.phase = phase;
            self.error = error;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_activate_emits_once() {
        let mut model = Model::default();
        assert_eq!(model.activate(), None);
        model.available();
        assert_eq!(model.activate(), Some(Effect::Download(1)));
        assert_eq!(model.activate(), None);
        assert_eq!(model.phase, Phase::Downloading);
        model.receive(1, Phase::Verifying, String::new());
        assert_eq!(model.activate(), None);
    }

    #[test]
    fn stale_completion_is_ignored_and_retry_has_a_new_identity() {
        let mut model = Model::default();
        model.available();
        model.activate();
        model.receive(1, Phase::Error, "Connection lost".into());
        assert_eq!(model.error, "Connection lost");
        assert_eq!(model.activate(), Some(Effect::Download(2)));
        model.receive(1, Phase::Ready, String::new());
        assert_eq!(model.phase, Phase::Downloading);
        assert!(model.error.is_empty());
    }

    #[test]
    fn completion_requires_explicit_apply_and_duplicate_apply_is_ignored() {
        let mut model = Model::default();
        model.available();
        model.activate();
        model.receive(1, Phase::Verifying, String::new());
        model.receive(1, Phase::Ready, String::new());
        assert_eq!(model.phase, Phase::Ready);
        assert_eq!(model.activate(), Some(Effect::Apply(1)));
        assert_eq!(model.activate(), None);
    }

    #[test]
    fn out_of_order_results_cannot_skip_verification() {
        let mut model = Model::default();
        model.available();
        model.activate();
        model.receive(1, Phase::Ready, String::new());
        assert_eq!(model.phase, Phase::Downloading);
    }

    #[test]
    fn failed_manual_reveal_retains_ready_and_allows_an_explicit_retry() {
        let mut model = Model::default();
        model.available();
        model.activate();
        model.receive(1, Phase::Verifying, String::new());
        model.receive(1, Phase::Ready, String::new());
        model.activate();
        model.applied(1, "File manager unavailable".into());
        assert_eq!(model.phase, Phase::Ready);
        assert!(!model.applying);
        assert_eq!(model.error, "File manager unavailable");
        assert_eq!(model.activate(), Some(Effect::Apply(1)));
    }
}
