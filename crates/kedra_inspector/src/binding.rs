//! Immutable consumer-owned value bindings, unrelated to authoring document types.
use bevy::reflect::{ParsedPath, PartialReflect};
use kedra_ui::fields::FieldAccess;
use std::any::TypeId;

/// Opaque host token. A different target or source generation requires a new token.
/// Equality is meaningful only within the host owning the inspected view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BindingToken(pub u64);

/// Optional host-supplied context, without source/document types in the inspector.
pub trait ValueMetadata: Send + Sync {
    /// Exact source-qualified path recorded for a native asset handle. No runtime
    /// UUID formatting or guessed path is a substitute. `None` means unavailable;
    /// this display lookup grants no resource-assignment capability.
    fn asset_path(&self, _handle: &bevy::asset::UntypedHandle) -> Option<String> {
        None
    }
    /// Source-qualified candidates admitted for this native asset type by the
    /// host's current immutable catalog/delivery. Assignment still revalidates
    /// source revisions, membership, type and delivery before publication.
    fn asset_choices(&self, _asset_type: TypeId) -> Vec<String> {
        Vec::new()
    }
    /// Visual type category from the host's existing field kit, not assignment
    /// authority. Unknown resources use a generic file icon.
    fn asset_icon(&self, _asset_type: TypeId) -> kedra_ui::fields::AssetIcon {
        kedra_ui::fields::AssetIcon::File
    }
    /// Human-readable provenance or ownership for a native path.
    fn describe(&self, path: Option<&ParsedPath>) -> Option<String>;
    /// Explain a host-specific representation which the generic editor cannot
    /// faithfully present. A custom editor may supply an explicit representation.
    fn limitation(
        &self,
        _value: &dyn PartialReflect,
        _path: Option<&ParsedPath>,
    ) -> Option<String> {
        None
    }
}

/// Borrowed value and presentation capabilities. The host owns all referenced data.
/// Callbacks must not retain these borrows beyond their mount/update call.
#[derive(Clone, Copy)]
pub struct ValueBinding<'a> {
    /// Host binding/revision identity, not an entity handle or label.
    pub token: BindingToken,
    /// Immutable native reflected value.
    pub value: &'a dyn PartialReflect,
    /// Associated display label, not discovery identity.
    pub label: &'a str,
    /// Explicit access and reason, shared by default and specialized controls.
    pub access: &'a FieldAccess,
    /// Native containing root type, for explicit field specialization.
    pub owner_type: Option<TypeId>,
    /// Native field path, absent for membership-based collection entries.
    pub path: Option<&'a ParsedPath>,
    /// Optional host diagnostic/provenance, not interpreted as write permission.
    pub diagnostic: Option<&'a str>,
    /// Consumer-owned provenance and representation limitations, never write authority.
    pub metadata: Option<&'a dyn ValueMetadata>,
}
