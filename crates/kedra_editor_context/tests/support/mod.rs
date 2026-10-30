use kedra_authoring::{
    persistence::{SourceAsset, SourceFile, SourceSnapshot, content_hash},
    workspace::WorkingSet,
};
use kedra_editor_context::{AssetLibrary, ScopeId};
use kedra_scene::document::{Node, NodeId, PrefabDocument};
use std::collections::BTreeMap;

pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
pub struct Fixture {
    pub scope: ScopeId,
    pub source: SourceSnapshot,
    pub working: WorkingSet,
    pub document: PrefabDocument,
    pub child: NodeId,
}
impl Fixture {
    pub fn new() -> Result<Self> {
        let mut document = PrefabDocument::new("Source");
        let child = NodeId::new();
        document.nodes.push(Node {
            id: child,
            name: "Child".into(),
            parent: Some(document.root.into()),
            components: BTreeMap::new(),
        });
        let text = document.serialize()?;
        let source = SourceSnapshot {
            environments: Default::default(),
            namespace: "game".into(),
            generation: 0,
            index: None,
            documents: vec![SourceFile {
                path: "Scenes/Main.prefab.ron".into(),
                hash: content_hash(&text),
                source: text,
            }],
            assets: vec![SourceAsset {
                id: Some(uuid::Uuid::new_v4()),
                path: "Images/Маяк.png".into(),
                hash: "baseline".into(),
                bytes: 0,
            }],
        };
        let mut working = WorkingSet::default();
        working.open(document.clone())?;
        Ok(Self {
            scope: ScopeId::default(),
            source,
            working,
            document,
            child,
        })
    }
    pub fn library(&self) -> AssetLibrary<'_> {
        AssetLibrary::new(self.scope, &self.source, &self.working)
    }
}
