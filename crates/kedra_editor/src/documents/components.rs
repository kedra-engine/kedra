//! Component commands retain their original source target and revision inventory.
use super::*;
use kedra_scene::document::ComponentData;

#[derive(Clone)]
pub(crate) struct ComponentCommand {
    pub target: inspection::Target,
    pub expected: BTreeMap<DocumentId, u64>,
    pub key: String,
    pub add: bool,
}

pub(crate) fn constructor(
    key: &str,
    types: &bevy::reflect::TypeRegistry,
    policies: &ComponentPolicies,
) -> Option<Box<dyn PartialReflect>> {
    let policy = policies.iter().find(|policy| policy.key == key)?;
    types
        .get_type_data::<bevy::reflect::std_traits::ReflectDefault>(policy.type_id)
        .map(|factory| factory.default().into_partial_reflect())
}

impl Documents {
    pub(crate) fn component_commands(
        &self,
        target: &inspection::Target,
        policies: &ComponentPolicies,
    ) -> Result<Vec<ComponentCommand>, String> {
        let inspection = self.inspect(target)?;
        let make = |key: &str, add| ComponentCommand {
            target: target.clone(),
            expected: self.revisions(),
            key: key.into(),
            add,
        };
        let mut commands: Vec<_> = inspection
            .components
            .keys()
            .map(|key| make(key, false))
            .collect();
        commands.extend(
            policies
                .iter()
                .filter(|policy| !inspection.components.contains_key(&policy.key))
                .map(|policy| make(&policy.key, true)),
        );
        Ok(commands)
    }
    pub(crate) fn change_component(
        &mut self,
        command: &ComponentCommand,
        types: &bevy::reflect::TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<(), String> {
        if self.revisions() != command.expected
            || self.creating()
            || self.opening()
            || self.save_busy()
            || self.save_pending()
        {
            return Err("Component command is stale or a document operation is pending.".into());
        }
        self.inspect(&command.target)?;
        let scope = self.scope.as_ref().ok_or("Source workspace unavailable")?;
        let path = format!(
            "{}://{}",
            scope.source.namespace,
            scope
                .path(command.target.document)
                .ok_or("Source path unavailable")?
        );
        let mut edit = SourceEdit::begin(
            scope,
            &format!(
                "{} {}",
                if command.add { "Add" } else { "Remove" },
                command.key
            ),
            std::slice::from_ref(&path),
        )?;
        let node = edit
            .nodes(scope, &path)?
            .into_iter()
            .find(|node| node.address == command.target.node)
            .ok_or("Component target was removed")?;
        if command.add {
            let value = constructor(&command.key, types, policies).ok_or("Component has no registered reflected default; an explicit initial value is required")?;
            let data: ComponentData = policies
                .encode(&command.key, value.as_ref(), types, &BTreeMap::new())
                .map_err(|error| error.to_string())?;
            edit.add_component(scope, &node, &command.key, data, types, policies)?;
        } else {
            edit.remove_component(scope, &node, &command.key, types, policies)?;
        }
        self.commit_edit(&mut edit, types, policies)?;
        Ok(())
    }
}
