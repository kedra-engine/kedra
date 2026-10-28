//! Complete native replacement for compound values: reflection apply is not list resize.
use super::*;
use bevy::reflect::{ReflectFromReflect, ReflectRef};

impl PropertySession {
    pub(super) fn lift(
        &self,
        working: &WorkingSet,
        proposals: &[Proposal],
        types: &TypeRegistry,
        policies: &ComponentPolicies,
    ) -> Result<Vec<Proposal>> {
        let document = &working
            .documents()
            .get(&self.target.document)
            .ok_or_else(|| failure("property document is unavailable"))?
            .document;
        let compound =
            proposals.iter().any(|proposal| {
                proposal.path.0.is_empty()
                    || proposal.path.0.iter().any(|part| {
                        matches!(part.access, Access::ListIndex(_) | Access::FieldIndex(_))
                    })
                    || !matches!(proposal.native.as_ref().reflect_ref(), ReflectRef::Opaque(_))
                || document.overrides.iter().any(|operation| {
                    if matches!(operation, Operation::SetComponent { node, component, .. } if *node == self.target.node && *component == self.target.component) {
                        return true;
                    }
                        if let Operation::SetField {
                            node,
                            component,
                            path,
                            ..
                        } = operation
                            && *node == self.target.node
                            && *component == self.target.component
                            && let Ok(path) = ParsedPath::parse(path)
                        {
                            path.0.len() < proposal.path.0.len() && overlaps(&path, &proposal.path)
                        } else {
                            false
                        }
                    })
            });
        if !compound {
            return Ok(proposals.to_vec());
        }

        // Reconstruct optional ports before cloning; copying a runtime-lowered None
        // here would silently erase authored symbolic identity in an unrelated field.
        let symbolic = self.snapshot.symbolic_source(types, policies)?;
        let baseline = component_value(&symbolic, &self.target, types, policies)?;
        let info = baseline
            .get_represented_type_info()
            .ok_or_else(|| failure("component type unavailable"))?;
        let mut value = types
            .get_type_data::<ReflectFromReflect>(info.type_id())
            .and_then(|factory| factory.from_reflect(baseline))
            .ok_or_else(|| failure("component cannot be cloned for complete replacement"))?;
        for proposal in proposals {
            let target = proposal
                .path
                .reflect_element_mut(value.as_partial_reflect_mut())
                .map_err(|error| failure(error.to_string()))?;
            let info = target
                .get_represented_type_info()
                .ok_or_else(|| failure("field type unavailable"))?;
            // Native handles deliberately implement reflection cloning without
            // registering ReflectFromReflect type data. Preserve their owned
            // handle/path instead of first erasing it into a dynamic enum.
            let replacement = proposal
                .native
                .as_ref()
                .reflect_clone()
                .ok()
                .or_else(|| {
                    types
                        .get_type_data::<ReflectFromReflect>(info.type_id())
                        .and_then(|factory| factory.from_reflect(proposal.native.as_ref()))
                })
                .ok_or_else(|| failure("field cannot be constructed for exact replacement"))?;
            target
                .try_as_reflect_mut()
                .ok_or_else(|| failure("field has no native replacement capability"))?
                .set(replacement)
                .map_err(|_| failure("field replacement type mismatch"))?;
        }
        let encoded = symbolic.encode_value(value.as_partial_reflect(), policies, types)?;
        Ok(vec![Proposal {
            path: ParsedPath::from(Vec::<Access<'static>>::new()),
            value: encoded,
            native: Arc::from(value.into_partial_reflect()),
        }])
    }
}
