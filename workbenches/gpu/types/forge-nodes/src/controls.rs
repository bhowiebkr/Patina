//! Exposed material controls and named numeric parameter snapshots.
use crate::{Graph, NodeId, ParamType, ParameterPreset, Registry};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("Preset name must contain 1 to 256 characters")]
    Name,
    #[error("Expose a numeric parameter before saving a preset")]
    Empty,
    #[error("The selected preset no longer exists")]
    Missing,
    #[error("Preset parameter {0} is missing or invalid")]
    Parameter(String),
}

impl Graph {
    /// Saving the same name deliberately updates that preset; it does not change evaluation.
    pub fn save_parameter_preset(
        &mut self,
        registry: &Registry,
        name: &str,
    ) -> Result<(), PresetError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 256 {
            return Err(PresetError::Name);
        }
        let mut values = BTreeMap::new();
        for control in &self.exposed {
            let key = format!("{}/{}", control.node.0, control.param);
            let node = self
                .node(control.node)
                .ok_or_else(|| PresetError::Parameter(key.clone()))?;
            let param = registry
                .get(&node.kind)
                .and_then(|def| def.param(&control.param))
                .filter(|param| !matches!(param.ty, ParamType::Text(_)))
                .ok_or_else(|| PresetError::Parameter(key.clone()))?;
            values.insert(
                key,
                param.pack(node.params.get(&control.param).map(Vec::as_slice)),
            );
        }
        if values.is_empty() {
            return Err(PresetError::Empty);
        }
        let preset = ParameterPreset {
            name: name.into(),
            values,
        };
        if let Some(existing) = self.presets.iter_mut().find(|preset| preset.name == name) {
            *existing = preset;
        } else {
            self.presets.push(preset);
        }
        Ok(())
    }

    /// Validate every entry first so a stale or incompatible preset cannot partially apply.
    pub fn apply_parameter_preset(
        &mut self,
        registry: &Registry,
        index: usize,
    ) -> Result<Vec<NodeId>, PresetError> {
        let preset = self.presets.get(index).ok_or(PresetError::Missing)?;
        let mut edits = Vec::new();
        for (key, value) in &preset.values {
            let invalid = || PresetError::Parameter(key.clone());
            let (id, name) = key.split_once('/').ok_or_else(invalid)?;
            let id = NodeId(id.parse().map_err(|_| invalid())?);
            let node = self.node(id).ok_or_else(invalid)?;
            let parameter = registry
                .get(&node.kind)
                .and_then(|def| def.param(name))
                .filter(|param| !matches!(param.ty, ParamType::Text(_)))
                .ok_or_else(invalid)?;
            if value.is_empty() || value.iter().any(|v| !v.is_finite()) {
                return Err(invalid());
            }
            let packed = parameter.pack(Some(value));
            if packed.len() != value.len() {
                return Err(invalid());
            }
            edits.push((id, name.to_owned(), packed));
        }
        let mut changed = BTreeSet::new();
        for (id, name, value) in edits {
            if let Some(node) = self.node_mut(id)
                && node.value(registry, &name) != value
            {
                node.params.insert(name, value);
                changed.insert(id);
            }
        }
        Ok(changed.into_iter().collect())
    }

    pub fn validate_controls(&self) -> Result<(), String> {
        if self.exposed.len() > 10000 || self.presets.len() > 1024 {
            return Err("Too many material controls or presets".into());
        }
        let mut exposed = BTreeSet::new();
        for e in &self.exposed {
            if self.node(e.node).is_none()
                || e.param.is_empty()
                || e.param.len() > 1024
                || e.label.len() > 4096
                || !exposed.insert((e.node, &e.param))
            {
                return Err("Invalid exposed material control".into());
            }
        }
        let mut names = BTreeSet::new();
        for preset in &self.presets {
            if preset.name.trim().is_empty()
                || preset.name.chars().count() > 256
                || !names.insert(&preset.name)
                || preset.values.len() > 10000
            {
                return Err("Invalid parameter preset".into());
            }
            for (key, value) in &preset.values {
                let valid_key = key.split_once('/').is_some_and(|(id, param)| {
                    id.parse()
                        .ok()
                        .is_some_and(|id| self.node(NodeId(id)).is_some())
                        && !param.is_empty()
                        && param.len() <= 1024
                });
                if !valid_key
                    || value.is_empty()
                    || value.len() > 4096
                    || value.iter().any(|v| !v.is_finite())
                {
                    return Err("Invalid parameter preset value".into());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Exposed;

    #[test]
    fn presets_restore_values_atomically_and_survive_serialization()
    -> Result<(), Box<dyn std::error::Error>> {
        let registry = Registry::embedded();
        let mut graph = crate::blank_material(&registry, "Controls");
        let id = graph.add("material", [0., 0.]);
        graph.exposed.push(Exposed {
            node: id,
            param: "basecolor_value".into(),
            label: "Paint".into(),
        });
        graph
            .node_mut(id)
            .ok_or("Missing node")?
            .params
            .insert("basecolor_value".into(), vec![0.2, 0.3, 0.4, 1.]);
        graph.save_parameter_preset(&registry, "Painted")?;
        let saved = graph.node(id).ok_or("Missing node")?.params.clone();
        graph.node_mut(id).ok_or("Missing node")?.params.clear();
        assert_eq!(graph.apply_parameter_preset(&registry, 0)?, vec![id]);
        assert_eq!(graph.node(id).ok_or("Missing node")?.params, saved);
        graph.save_parameter_preset(&registry, "Painted")?;
        assert_eq!(graph.presets.len(), 1);
        let mut loaded: Graph = serde_json::from_slice(&serde_json::to_vec(&graph)?)?;
        assert_eq!(loaded, graph);
        loaded.presets[0]
            .values
            .insert("999/missing".into(), vec![0.]);
        loaded.node_mut(id).ok_or("Missing node")?.params.clear();
        let before = loaded.clone();
        assert!(loaded.apply_parameter_preset(&registry, 0).is_err());
        assert_eq!(loaded, before);
        assert!(loaded.validate_controls().is_err());
        graph.remove(id);
        assert!(graph.exposed.is_empty());
        assert!(graph.presets.is_empty());
        graph.validate_controls()?;
        Ok(())
    }
}
