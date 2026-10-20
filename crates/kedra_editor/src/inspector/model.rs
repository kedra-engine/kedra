//! View-local following and locking over persistent source target identities.
use crate::documents::inspection::Target;

#[derive(Default)]
pub(crate) struct State {
    pub target: Option<Target>,
    pub locked: bool,
    pub token: u64,
    pub raw: bool,
    pub search_open: bool,
    pub details_open: bool,
}
impl State {
    pub fn follow(&mut self, selected: Option<Target>) -> Result<bool, String> {
        if self.locked || self.target == selected {
            return Ok(false);
        }
        self.token = self
            .token
            .checked_add(1)
            .ok_or("Inspector binding counter exhausted")?;
        self.target = selected;
        Ok(true)
    }
    pub fn toggle_lock(&mut self) {
        if self.locked || self.target.is_some() {
            self.locked = !self.locked;
        }
    }
}
