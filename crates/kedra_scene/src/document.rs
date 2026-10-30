//! Versioned authored sources. Runtime entity handles are never persisted here.
use crate::{Result, failure};
use bevy::prelude::*;
use ron::value::RawValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

macro_rules! identity {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            Reflect,
        )]
        #[serde(transparent)]
        pub struct $name(
            #[doc = "Persistent UUID; nil is rejected when validating sources."] pub Uuid,
        );
        impl $name {
            /// Allocate a new identity. Copying an identity refers to the same source object.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
identity!(
    DocumentId,
    "Stable authored document identity, independent of live entities and paths."
);
identity!(NodeId, "Stable node identity within a source document.");
identity!(
    InstanceId,
    "Stable nesting scope in a document; runtime instances allocate distinct identities."
);

#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Reflect,
)]
#[serde(deny_unknown_fields)]
/// A source node plus its complete nested-instance scope, never a live entity handle.
pub struct NodeAddress {
    /// Outer-to-inner stable nesting identities.
    pub instances: Vec<InstanceId>,
    /// Node identity in the innermost source.
    pub node: NodeId,
}
impl From<NodeId> for NodeAddress {
    fn from(node: NodeId) -> Self {
        Self {
            instances: Vec::new(),
            node,
        }
    }
}
impl NodeAddress {
    /// Prepend a composition scope without changing this address.
    pub fn scoped(&self, prefix: &[InstanceId]) -> Self {
        Self {
            instances: prefix.iter().chain(&self.instances).copied().collect(),
            node: self.node,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Persisted reference, resolved and remapped when compiling/instantiating.
pub enum Reference {
    /// Node in this document's composition scope.
    Local(NodeAddress),
    /// Named external port declared by the document.
    Port(String),
}

/// The component's Rust type is registered once; source contains typed RON values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentData {
    /// Exact version required by the registered component policy.
    pub version: u32,
    /// Typed RON source, parsed by RON rather than coerced through untyped JSON.
    #[serde(deserialize_with = "raw_value")]
    pub value: Box<RawValue>,
}
impl ComponentData {
    /// Validate and retain a bounded RON value. No type construction or world writes occur.
    pub fn ron(version: u32, text: &str) -> Result<Self> {
        Ok(Self {
            version,
            value: crate::value::parse(text)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Source node. Parent links form one validated ordered hierarchy after composition.
pub struct Node {
    /// Stable identity; duplicates and nil IDs are rejected.
    pub id: NodeId,
    /// Human-readable source name, not a runtime lookup identity.
    pub name: String,
    /// Parent address; only the document root has no parent.
    /// Must name a live node directly, not a symbolic reference-remap alias.
    pub parent: Option<NodeAddress>,
    /// Explicitly allowed component schema keys and their typed source values.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub components: BTreeMap<String, ComponentData>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// One prefab composition site. Overrides apply only in this declared scope.
pub struct NestedInstance {
    /// Stable composition-site identity.
    pub id: InstanceId,
    /// Referenced prefab document, not a scene document.
    pub asset: DocumentId,
    /// Parent in the containing document.
    pub parent: NodeAddress,
    /// Explicit bindings for the prefab's external ports.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub bindings: BTreeMap<String, Reference>,
    /// Ordered source edits layered over the referenced prefab.
    pub overrides: Vec<Operation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Authored edits interpreted during composition, never arbitrary live ECS mutation.
pub enum Operation {
    /// Replace a reflected field. Overlapping patches in a layer are rejected.
    SetField {
        /// Target source node.
        node: NodeAddress,
        /// Registered component key.
        component: String,
        /// Exact component schema version.
        version: u32,
        /// Unambiguous reflected field path.
        path: String,
        /// Typed RON replacement value.
        #[serde(deserialize_with = "raw_value")]
        value: Box<RawValue>,
    },
    /// Replace a complete component and reset inherited field patches.
    SetComponent {
        /// Target source node.
        node: NodeAddress,
        /// Registered component key.
        component: String,
        /// Replacement component source.
        data: ComponentData,
    },
    /// Remove an existing component; missing targets are errors.
    RemoveComponent {
        /// Target source node.
        node: NodeAddress,
        /// Registered component key.
        component: String,
    },
    /// Add a new stable node; duplicate identities fail.
    AddNode(Node),
    /// Remove a non-root node and its descendants.
    RemoveNode(NodeAddress),
    /// Change only a source name.
    RenameNode {
        /// Target source node.
        node: NodeAddress,
        /// New source name.
        name: String,
    },
    /// Move within a prefab boundary, or move an entire nested instance root.
    /// Internal nodes cannot cross their instance boundary. Root moves retain
    /// the complete instance scope, source references and overrides; cycles fail.
    ReparentNode {
        /// Target source node.
        node: NodeAddress,
        /// New parent in the same scope.
        parent: NodeAddress,
    },
    /// Order every direct child exactly once.
    SetChildOrder {
        /// Parent source node.
        node: NodeAddress,
        /// Complete ordered child list.
        children: Vec<NodeAddress>,
    },
    /// Add a composition site and validate its dependencies.
    AddNestedInstance(NestedInstance),
    /// Remove a direct composition site.
    RemoveNestedInstance(InstanceId),
    /// Remove a composition subtree by complete scope.
    RemoveNestedInstanceAt(Vec<InstanceId>),
    /// Preserve source references during an explicit unpack/edit transaction.
    RemapReferences(
        #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
        BTreeMap<NodeAddress, NodeAddress>,
    ),
    /// Change one declared external binding.
    SetBinding {
        /// Direct composition site.
        instance: InstanceId,
        /// Declared port name.
        port: String,
        /// Reference in the containing source scope.
        target: Reference,
    },
}

fn raw_value<'de, D: serde::Deserializer<'de>>(
    de: D,
) -> std::result::Result<Box<RawValue>, D::Error> {
    let value = Box::<RawValue>::deserialize(de)?;
    crate::value::parse(value.get_ron()).map_err(serde::de::Error::custom)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Scenes are top-level entry points; only prefabs can be nested.
pub enum DocumentKind {
    /// Reusable composition source.
    Prefab,
    /// Top-level authored scene.
    Scene,
}

#[derive(Asset, TypePath, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Versioned authoring authority, loaded as a native Bevy asset.
/// Compiling or instantiating never mutates this source.
pub struct PrefabDocument {
    /// Document format; this implementation accepts version 1 only.
    pub version: u32,
    /// Stable document/asset identity, checked by the delivery producer.
    pub id: DocumentId,
    /// Top-level scene or reusable prefab.
    pub kind: DocumentKind,
    /// Human-readable authoring title.
    pub title: String,
    /// Root identity, unchanged by variants.
    pub root: NodeId,
    /// Optional base of the same document kind.
    pub base: Option<DocumentId>,
    /// Locally authored nodes.
    pub nodes: Vec<Node>,
    /// Direct nested composition sites.
    pub instances: Vec<NestedInstance>,
    /// Ordered source edits over the composed graph.
    pub overrides: Vec<Operation>,
    /// Named external ports: true requires a binding, false explicitly permits absence.
    #[serde(with = "serde_with::rust::maps_duplicate_key_is_error")]
    pub ports: BTreeMap<String, bool>,
}
impl PrefabDocument {
    /// Create an independent in-memory prefab with new document/root identities.
    pub fn new(title: impl Into<String>) -> Self {
        let root = NodeId::new();
        let title = title.into();
        Self {
            version: 1,
            id: DocumentId::new(),
            kind: DocumentKind::Prefab,
            nodes: vec![Node {
                id: root,
                name: title.clone(),
                parent: None,
                components: BTreeMap::new(),
            }],
            title,
            root,
            base: None,
            instances: Vec::new(),
            overrides: Vec::new(),
            ports: BTreeMap::new(),
        }
    }
    /// Parse a bounded source and reject incompatible versions/nil roots.
    /// Graph and component validation happen before publication, not by mutating a live world.
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 65_536 {
            return Err(failure("scene source exceeds 64 KiB"));
        }
        let document: Self = ron::from_str(text)?;
        if document.version != 1 {
            return Err(failure(
                "unsupported prefab document version; source must remain read-only",
            ));
        }
        if document.id.0.is_nil() || document.root.0.is_nil() {
            return Err(failure("document and root IDs cannot be nil"));
        }
        Ok(document)
    }
    /// Serialize authored data with RON. Never serializes a live ECS instance.
    pub fn serialize(&self) -> Result<String> {
        Ok(ron::ser::to_string_pretty(
            self,
            ron::ser::PrettyConfig::default(),
        )?)
    }
    /// Create an independent variant source referring to this base and retaining its root ID.
    pub fn variant(&self, title: impl Into<String>) -> Self {
        Self {
            version: 1,
            id: DocumentId::new(),
            kind: self.kind,
            title: title.into(),
            root: self.root,
            base: Some(self.id),
            nodes: Vec::new(),
            instances: Vec::new(),
            overrides: Vec::new(),
            ports: BTreeMap::new(),
        }
    }
}
