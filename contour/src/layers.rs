use crate::model::{Group, Layer, LayerId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manages layers and groups for organizing edges hierarchically
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LayerSystem {
    pub layers: Vec<Layer>,
    pub groups: HashMap<LayerId, Group>,
    pub edge_to_group: HashMap<u32, LayerId>,
    pub(crate) next_id: LayerId,
}

impl LayerSystem {
    pub fn new() -> Self {
        let mut sys = LayerSystem::default();
        sys.create_layer("Layer 1".to_string());
        sys
    }

    fn next_id(&mut self) -> LayerId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Create a new layer with a root group, returns layer ID
    pub fn create_layer(&mut self, name: String) -> LayerId {
        let layer_id = self.next_id();
        let root_group_id = self.next_id();

        let root_group = Group {
            id: root_group_id,
            name: format!("{} (root)", name),
            parent: None,
            children: Vec::new(),
            edges: Vec::new(),
            visible: true,
            locked: false,
            opacity: 1.0,
        };

        let z = self.layers.len() as i32;
        let layer = Layer {
            id: layer_id,
            name,
            z_index: z,
            visible: true,
            locked: false,
            opacity: 1.0,
            root_group: root_group_id,
        };

        self.groups.insert(root_group_id, root_group);
        self.layers.push(layer);
        layer_id
    }

    /// Remove a layer and all its groups, returns removed edge IDs
    pub fn remove_layer(&mut self, id: LayerId) -> Option<Vec<u32>> {
        let idx = self.layers.iter().position(|l| l.id == id)?;
        let layer = self.layers.remove(idx);

        let mut removed_edges = Vec::new();
        let mut groups_to_remove = vec![layer.root_group];

        while let Some(gid) = groups_to_remove.pop() {
            if let Some(group) = self.groups.remove(&gid) {
                removed_edges.extend(group.edges.iter().copied());
                groups_to_remove.extend(group.children.iter().copied());

                for eid in &group.edges {
                    self.edge_to_group.remove(eid);
                }
            }
        }

        Some(removed_edges)
    }

    /// Get layer by ID
    pub fn get_layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Get mutable layer by ID
    pub fn get_layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// Get the default layer's root group ID (for assigning new edges)
    pub fn default_group(&self) -> Option<LayerId> {
        self.layers.first().map(|l| l.root_group)
    }

    /// Create a group within a parent group, returns group ID
    pub fn create_group(&mut self, name: String, parent_id: LayerId) -> Option<LayerId> {
        if !self.groups.contains_key(&parent_id) {
            return None;
        }

        let group_id = self.next_id();
        let group = Group {
            id: group_id,
            name,
            parent: Some(parent_id),
            children: Vec::new(),
            edges: Vec::new(),
            visible: true,
            locked: false,
            opacity: 1.0,
        };

        self.groups.insert(group_id, group);
        if let Some(parent) = self.groups.get_mut(&parent_id) {
            parent.children.push(group_id);
        }

        Some(group_id)
    }

    /// Remove a group and reassign its edges/children to parent
    pub fn remove_group(&mut self, id: LayerId) -> bool {
        let group = match self.groups.remove(&id) {
            Some(g) => g,
            None => return false,
        };

        // Can't remove root groups (those without parents)
        let parent_id = match group.parent {
            Some(p) => p,
            None => {
                // Put it back
                self.groups.insert(id, group);
                return false;
            }
        };

        // Collect children to reparent
        let children_to_reparent: Vec<LayerId> = group.children.clone();

        // Move edges to parent and update parent's children list
        if let Some(parent) = self.groups.get_mut(&parent_id) {
            parent.children.retain(|&c| c != id);
            for eid in &group.edges {
                parent.edges.push(*eid);
            }
            // Add removed group's children to parent
            for child_id in &children_to_reparent {
                parent.children.push(*child_id);
            }
        }

        // Update edge-to-group mappings
        for eid in &group.edges {
            self.edge_to_group.insert(*eid, parent_id);
        }

        // Reparent children (separate borrow)
        for child_id in &children_to_reparent {
            if let Some(child) = self.groups.get_mut(child_id) {
                child.parent = Some(parent_id);
            }
        }

        true
    }

    /// Get group by ID
    pub fn get_group(&self, id: LayerId) -> Option<&Group> {
        self.groups.get(&id)
    }

    /// Get mutable group by ID
    pub fn get_group_mut(&mut self, id: LayerId) -> Option<&mut Group> {
        self.groups.get_mut(&id)
    }

    /// Add an edge to a group
    pub fn add_edge_to_group(&mut self, edge_id: u32, group_id: LayerId) -> bool {
        // Remove from previous group if any
        if let Some(old_group_id) = self.edge_to_group.remove(&edge_id) {
            if let Some(old_group) = self.groups.get_mut(&old_group_id) {
                old_group.edges.retain(|&e| e != edge_id);
            }
        }

        if let Some(group) = self.groups.get_mut(&group_id) {
            group.edges.push(edge_id);
            self.edge_to_group.insert(edge_id, group_id);
            true
        } else {
            false
        }
    }

    /// Remove an edge from its group
    pub fn remove_edge(&mut self, edge_id: u32) {
        if let Some(group_id) = self.edge_to_group.remove(&edge_id) {
            if let Some(group) = self.groups.get_mut(&group_id) {
                group.edges.retain(|&e| e != edge_id);
            }
        }
    }

    /// Get the group containing an edge
    pub fn get_edge_group(&self, edge_id: u32) -> Option<LayerId> {
        self.edge_to_group.get(&edge_id).copied()
    }

    /// Find which layer a group belongs to
    pub fn find_layer_for_group(&self, group_id: LayerId) -> Option<LayerId> {
        let mut current = group_id;
        loop {
            let group = self.groups.get(&current)?;
            match group.parent {
                Some(parent) => current = parent,
                None => {
                    // Found root group, find its layer
                    return self
                        .layers
                        .iter()
                        .find(|l| l.root_group == current)
                        .map(|l| l.id);
                }
            }
        }
    }

    /// Get the layer containing an edge
    pub fn get_edge_layer(&self, edge_id: u32) -> Option<LayerId> {
        let group_id = self.edge_to_group.get(&edge_id)?;
        self.find_layer_for_group(*group_id)
    }

    /// Get layers in z-order (bottom to top)
    pub fn layers_ordered(&self) -> Vec<&Layer> {
        let mut layers: Vec<_> = self.layers.iter().collect();
        layers.sort_by_key(|l| l.z_index);
        layers
    }

    /// Check if an edge is visible (considering layer and group visibility chain)
    pub fn is_edge_visible(&self, edge_id: u32) -> bool {
        let group_id = match self.edge_to_group.get(&edge_id) {
            Some(gid) => *gid,
            None => return true, // Edges without group are visible by default
        };

        let mut current = group_id;
        loop {
            let group = match self.groups.get(&current) {
                Some(g) => g,
                None => return true,
            };

            if !group.visible {
                return false;
            }

            match group.parent {
                Some(parent) => current = parent,
                None => {
                    // Check layer visibility
                    if let Some(layer) = self.layers.iter().find(|l| l.root_group == current) {
                        return layer.visible;
                    }
                    return true;
                }
            }
        }
    }

    /// Compute effective opacity for an edge (accumulates through chain)
    pub fn edge_opacity(&self, edge_id: u32) -> f32 {
        let group_id = match self.edge_to_group.get(&edge_id) {
            Some(gid) => *gid,
            None => return 1.0,
        };

        let mut opacity = 1.0f32;
        let mut current = group_id;

        loop {
            let group = match self.groups.get(&current) {
                Some(g) => g,
                None => break,
            };

            opacity *= group.opacity;

            match group.parent {
                Some(parent) => current = parent,
                None => {
                    if let Some(layer) = self.layers.iter().find(|l| l.root_group == current) {
                        opacity *= layer.opacity;
                    }
                    break;
                }
            }
        }

        opacity.clamp(0.0, 1.0)
    }

    /// Set layer visibility
    pub fn set_layer_visibility(&mut self, id: LayerId, visible: bool) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) {
            layer.visible = visible;
            true
        } else {
            false
        }
    }

    /// Set layer opacity
    pub fn set_layer_opacity(&mut self, id: LayerId, opacity: f32) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) {
            layer.opacity = opacity.clamp(0.0, 1.0);
            true
        } else {
            false
        }
    }

    /// Set layer z-index
    pub fn set_layer_z_index(&mut self, id: LayerId, z: i32) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) {
            layer.z_index = z;
            true
        } else {
            false
        }
    }

    /// Rename a layer
    pub fn rename_layer(&mut self, id: LayerId, name: String) -> bool {
        if let Some(layer) = self.layers.iter_mut().find(|l| l.id == id) {
            layer.name = name;
            true
        } else {
            false
        }
    }

    /// Set group visibility
    pub fn set_group_visibility(&mut self, id: LayerId, visible: bool) -> bool {
        if let Some(group) = self.groups.get_mut(&id) {
            group.visible = visible;
            true
        } else {
            false
        }
    }

    /// Set group opacity
    pub fn set_group_opacity(&mut self, id: LayerId, opacity: f32) -> bool {
        if let Some(group) = self.groups.get_mut(&id) {
            group.opacity = opacity.clamp(0.0, 1.0);
            true
        } else {
            false
        }
    }

    /// Rename a group
    pub fn rename_group(&mut self, id: LayerId, name: String) -> bool {
        if let Some(group) = self.groups.get_mut(&id) {
            group.name = name;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_layer() {
        let mut sys = LayerSystem::new();
        assert_eq!(sys.layers.len(), 1);
        assert_eq!(sys.layers[0].name, "Layer 1");

        let id = sys.create_layer("Layer 2".to_string());
        assert_eq!(sys.layers.len(), 2);
        assert!(sys.get_layer(id).is_some());
    }

    #[test]
    fn test_create_group() {
        let mut sys = LayerSystem::new();
        let root = sys.default_group().unwrap();

        let gid = sys.create_group("My Group".to_string(), root).unwrap();
        assert!(sys.get_group(gid).is_some());
        assert_eq!(sys.get_group(gid).unwrap().name, "My Group");
    }

    #[test]
    fn test_edge_to_group() {
        let mut sys = LayerSystem::new();
        let root = sys.default_group().unwrap();

        sys.add_edge_to_group(0, root);
        assert_eq!(sys.get_edge_group(0), Some(root));

        let gid = sys.create_group("G".to_string(), root).unwrap();
        sys.add_edge_to_group(0, gid);
        assert_eq!(sys.get_edge_group(0), Some(gid));
    }

    #[test]
    fn test_visibility_chain() {
        let mut sys = LayerSystem::new();
        let layer_id = sys.layers[0].id;
        let root = sys.default_group().unwrap();

        let g1 = sys.create_group("G1".to_string(), root).unwrap();
        let g2 = sys.create_group("G2".to_string(), g1).unwrap();

        sys.add_edge_to_group(0, g2);

        assert!(sys.is_edge_visible(0));

        sys.set_group_visibility(g1, false);
        assert!(!sys.is_edge_visible(0));

        sys.set_group_visibility(g1, true);
        assert!(sys.is_edge_visible(0));

        sys.set_layer_visibility(layer_id, false);
        assert!(!sys.is_edge_visible(0));
    }

    #[test]
    fn test_opacity_chain() {
        let mut sys = LayerSystem::new();
        let layer_id = sys.layers[0].id;
        let root = sys.default_group().unwrap();

        let g1 = sys.create_group("G1".to_string(), root).unwrap();
        sys.add_edge_to_group(0, g1);

        sys.set_layer_opacity(layer_id, 0.8);
        sys.set_group_opacity(g1, 0.5);

        let opacity = sys.edge_opacity(0);
        assert!((opacity - 0.4).abs() < 0.001);
    }
}
