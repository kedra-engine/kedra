//! Bounded, session-local asynchronous receipts. Features still own their tasks.
use bevy::prelude::*;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Outcome of an admitted operation, never inferred from HTTP acceptance.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    /// Work has been admitted but has not completed.
    Pending,
    /// The feature confirmed completion.
    Succeeded,
    /// The feature refused or failed the operation.
    Failed,
}
/// Readable receipt. The digest prevents reusing an ID for different arguments.
#[derive(Clone, Debug, Serialize)]
pub struct Operation {
    /// Caller-supplied idempotency key.
    pub id: Uuid,
    /// Canonical serialized-request checksum.
    pub request_hash: String,
    /// Actual execution state.
    pub state: OperationState,
    /// Result or actionable failure details.
    pub detail: Value,
}
/// At most 256 receipts. Completed receipts expire oldest-first; active receipts
/// are never evicted. Idempotency is session-local within this advertised window.
#[derive(Resource, Default)]
pub struct Operations {
    entries: BTreeMap<Uuid, Operation>,
    order: std::collections::VecDeque<Uuid>,
}
impl Operations {
    /// Admit a request, returning false for a matching retry. A conflicting ID,
    /// or a full collection of active operations, is rejected without effects.
    pub fn admit(&mut self, id: Uuid, request: &Value) -> Result<bool, String> {
        let hash = kedra_assets::delivery::digest(request.to_string().as_bytes());
        if id.is_nil() {
            return Err("Operation ID must not be nil".into());
        }
        if let Some(old) = self.entries.get(&id) {
            return if old.request_hash == hash {
                Ok(false)
            } else {
                Err("Operation ID was already used for another request".into())
            };
        }
        if self.entries.len() == 256 {
            let position = self
                .order
                .iter()
                .position(|id| {
                    self.entries
                        .get(id)
                        .is_some_and(|op| op.state != OperationState::Pending)
                })
                .ok_or("All operation slots are active")?;
            if let Some(id) = self.order.remove(position) {
                self.entries.remove(&id);
            }
        }
        self.order.push_back(id);
        self.entries.insert(
            id,
            Operation {
                id,
                request_hash: hash,
                state: OperationState::Pending,
                detail: Value::Null,
            },
        );
        Ok(true)
    }
    /// Read without changing execution state.
    pub fn get(&self, id: Uuid) -> Option<&Operation> {
        self.entries.get(&id)
    }
    /// Complete exactly once; late/duplicate completions cannot replace a receipt.
    pub fn finish(&mut self, id: Uuid, result: Result<Value, String>) -> bool {
        let Some(op) = self
            .entries
            .get_mut(&id)
            .filter(|op| op.state == OperationState::Pending)
        else {
            return false;
        };
        match result {
            Ok(value) => {
                op.state = OperationState::Succeeded;
                op.detail = value;
            }
            Err(error) => {
                op.state = OperationState::Failed;
                op.detail = Value::String(error);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_conflicts_capacity_and_stale_results_are_explicit() {
        let mut ops = Operations::default();
        let id = Uuid::new_v4();
        let request = serde_json::json!({"name":"Game"});
        assert_eq!(ops.admit(id, &request), Ok(true));
        assert_eq!(ops.admit(id, &request), Ok(false));
        assert!(ops.admit(id, &Value::Null).is_err());
        assert!(ops.finish(id, Ok(Value::Bool(true))));
        assert!(!ops.finish(id, Err("late".into())));
        for _ in 0..256 {
            assert!(ops.admit(Uuid::new_v4(), &request).is_ok());
        }
        assert!(ops.admit(Uuid::new_v4(), &request).is_err());
        assert!(ops.admit(Uuid::nil(), &request).is_err());
    }
}
