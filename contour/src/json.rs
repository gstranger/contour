use crate::geometry::limits;
use crate::layers::LayerSystem;
use crate::{
    model::{
        Color, Effect, EffectId, EffectStack, FillState, FontStyle, Gradient, GradientId, Group,
        HandleMode, Layer, LayerId, TextAlign, TextElement, TextId, TextOverflow, TextStyle,
        TextType, Vec2, VerticalAlign,
    },
    Graph,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn to_json_impl(g: &Graph) -> Value {
    #[derive(Serialize)]
    struct NodeSer {
        id: u32,
        x: f32,
        y: f32,
    }
    #[derive(Serialize)]
    #[serde(tag = "kind", rename_all = "lowercase")]
    enum EdgeSerKind {
        Line,
        Cubic {
            ha: Vec2,
            hb: Vec2,
            mode: HandleMode,
        },
        Polyline {
            points: Vec<Vec2>,
        },
    }
    #[derive(Serialize)]
    struct EdgeSer {
        id: u32,
        a: u32,
        b: u32,
        #[serde(flatten)]
        kind: EdgeSerKind,
        stroke: Option<Color>,
        width: f32,
    }
    #[derive(Serialize)]
    struct FillSer {
        key: u32,
        filled: bool,
        color: Option<Color>,
    }
    #[derive(Serialize)]
    struct LayerSer {
        id: LayerId,
        name: String,
        z_index: i32,
        visible: bool,
        locked: bool,
        opacity: f32,
        root_group: LayerId,
    }
    #[derive(Serialize)]
    struct GroupSer {
        id: LayerId,
        name: String,
        parent: Option<LayerId>,
        children: Vec<LayerId>,
        edges: Vec<u32>,
        visible: bool,
        locked: bool,
        opacity: f32,
    }
    #[derive(Serialize)]
    struct GradientSer {
        id: GradientId,
        #[serde(flatten)]
        gradient: Gradient,
    }
    #[derive(Serialize)]
    struct EffectSer {
        id: EffectId,
        #[serde(flatten)]
        effect: Effect,
    }
    #[derive(Serialize)]
    struct EffectBindingSer {
        target_type: String,
        target_id: u32,
        effects: Vec<EffectId>,
        enabled: bool,
    }
    #[derive(Serialize)]
    struct Doc {
        version: u32,
        nodes: Vec<NodeSer>,
        edges: Vec<EdgeSer>,
        fills: Vec<FillSer>,
        layers: Vec<LayerSer>,
        groups: Vec<GroupSer>,
        gradients: Vec<GradientSer>,
        texts: Vec<TextElement>,
        effects: Vec<EffectSer>,
        effect_bindings: Vec<EffectBindingSer>,
    }
    let mut nodes = Vec::new();
    for (i, n) in g.nodes.iter().enumerate() {
        if let Some(n) = n {
            nodes.push(NodeSer {
                id: i as u32,
                x: n.x,
                y: n.y,
            });
        }
    }
    let mut edges = Vec::new();
    for (i, e) in g.edges.iter().enumerate() {
        if let Some(e) = e {
            let kind = match &e.kind {
                crate::model::EdgeKind::Line => EdgeSerKind::Line,
                crate::model::EdgeKind::Cubic { ha, hb, mode } => EdgeSerKind::Cubic {
                    ha: *ha,
                    hb: *hb,
                    mode: *mode,
                },
                crate::model::EdgeKind::Polyline { points } => EdgeSerKind::Polyline {
                    points: points.clone(),
                },
            };
            edges.push(EdgeSer {
                id: i as u32,
                a: e.a,
                b: e.b,
                kind,
                stroke: e.stroke,
                width: e.stroke_width,
            });
        }
    }
    let mut fills = Vec::new();
    for (k, v) in g.fills.iter() {
        fills.push(FillSer {
            key: *k,
            filled: v.filled,
            color: v.color,
        });
    }
    // Serialize layers
    let layers: Vec<LayerSer> = g
        .layer_system
        .layers
        .iter()
        .map(|l| LayerSer {
            id: l.id,
            name: l.name.clone(),
            z_index: l.z_index,
            visible: l.visible,
            locked: l.locked,
            opacity: l.opacity,
            root_group: l.root_group,
        })
        .collect();
    // Serialize groups
    let groups: Vec<GroupSer> = g
        .layer_system
        .groups
        .values()
        .map(|gr| GroupSer {
            id: gr.id,
            name: gr.name.clone(),
            parent: gr.parent,
            children: gr.children.clone(),
            edges: gr.edges.clone(),
            visible: gr.visible,
            locked: gr.locked,
            opacity: gr.opacity,
        })
        .collect();
    // Serialize gradients
    let gradients: Vec<GradientSer> = g
        .gradients
        .iter()
        .map(|(id, gradient)| GradientSer {
            id: *id,
            gradient: gradient.clone(),
        })
        .collect();
    // Serialize texts
    let texts: Vec<TextElement> = g
        .texts
        .iter()
        .filter_map(|t| t.clone())
        .collect();
    // Serialize effects
    let effects: Vec<EffectSer> = g
        .effects
        .iter()
        .map(|(id, effect)| EffectSer {
            id: *id,
            effect: effect.clone(),
        })
        .collect();
    // Serialize effect bindings
    let mut effect_bindings: Vec<EffectBindingSer> = Vec::new();
    for (id, stack) in &g.shape_effects {
        if !stack.effects.is_empty() {
            effect_bindings.push(EffectBindingSer {
                target_type: "shape".to_string(),
                target_id: *id,
                effects: stack.effects.clone(),
                enabled: stack.enabled,
            });
        }
    }
    for (id, stack) in &g.region_effects {
        if !stack.effects.is_empty() {
            effect_bindings.push(EffectBindingSer {
                target_type: "region".to_string(),
                target_id: *id,
                effects: stack.effects.clone(),
                enabled: stack.enabled,
            });
        }
    }
    for (id, stack) in &g.text_effects {
        if !stack.effects.is_empty() {
            effect_bindings.push(EffectBindingSer {
                target_type: "text".to_string(),
                target_id: *id,
                effects: stack.effects.clone(),
                enabled: stack.enabled,
            });
        }
    }
    for (id, stack) in &g.group_effects {
        if !stack.effects.is_empty() {
            effect_bindings.push(EffectBindingSer {
                target_type: "group".to_string(),
                target_id: *id,
                effects: stack.effects.clone(),
                enabled: stack.enabled,
            });
        }
    }
    serde_json::to_value(Doc {
        version: 4,
        nodes,
        edges,
        fills,
        layers,
        groups,
        gradients,
        texts,
        effects,
        effect_bindings,
    })
    .unwrap()
}

pub fn from_json_impl(g: &mut Graph, v: Value) -> bool {
    #[derive(Deserialize)]
    struct NodeDe {
        id: u32,
        x: f32,
        y: f32,
    }
    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "lowercase")]
    enum EdgeDeKind {
        Line,
        Cubic {
            ha: Vec2,
            hb: Vec2,
            mode: Option<HandleMode>,
        },
        Polyline {
            points: Vec<Vec2>,
        },
    }
    #[derive(Deserialize)]
    struct EdgeDe {
        id: u32,
        a: u32,
        b: u32,
        #[serde(flatten)]
        kind: Option<EdgeDeKind>,
        stroke: Option<Color>,
        width: Option<f32>,
    }
    #[derive(Deserialize)]
    struct FillDe {
        key: u32,
        filled: bool,
        color: Option<Color>,
    }
    #[derive(Deserialize)]
    struct LayerDe {
        id: LayerId,
        name: String,
        z_index: i32,
        visible: bool,
        locked: bool,
        opacity: f32,
        root_group: LayerId,
    }
    #[derive(Deserialize)]
    struct GroupDe {
        id: LayerId,
        name: String,
        parent: Option<LayerId>,
        children: Vec<LayerId>,
        edges: Vec<u32>,
        visible: bool,
        locked: bool,
        opacity: f32,
    }
    #[derive(Deserialize)]
    struct GradientDe {
        id: GradientId,
        #[serde(flatten)]
        gradient: Gradient,
    }
    #[derive(Deserialize)]
    struct EffectDe {
        id: EffectId,
        #[serde(flatten)]
        effect: Effect,
    }
    #[derive(Deserialize)]
    struct EffectBindingDe {
        target_type: String,
        target_id: u32,
        effects: Vec<EffectId>,
        enabled: bool,
    }
    #[derive(Deserialize)]
    struct DocDe {
        version: Option<u32>,
        nodes: Vec<NodeDe>,
        edges: Vec<EdgeDe>,
        fills: Option<Vec<FillDe>>,
        layers: Option<Vec<LayerDe>>,
        groups: Option<Vec<GroupDe>>,
        gradients: Option<Vec<GradientDe>>,
        texts: Option<Vec<TextElement>>,
        effects: Option<Vec<EffectDe>>,
        effect_bindings: Option<Vec<EffectBindingDe>>,
    }
    let parsed: Result<DocDe, _> = serde_json::from_value(v);
    if let Ok(doc) = parsed {
        // Caps: sizes
        if doc.nodes.len() > limits::MAX_NODES || doc.edges.len() > limits::MAX_EDGES {
            return false;
        }
        // Validate nodes
        for n in &doc.nodes {
            if !limits::in_coord_bounds(n.x) || !limits::in_coord_bounds(n.y) {
                return false;
            }
        }
        // Validate edges preliminarily
        let mut poly_total: usize = 0;
        for e in &doc.edges {
            if e.a == e.b {
                return false;
            }
            if let Some(kind) = &e.kind {
                match kind {
                    EdgeDeKind::Line => {}
                    EdgeDeKind::Cubic { ha, hb, .. } => {
                        // ha/hb are offsets; ensure absolute positions would be in bounds if endpoints in bounds
                        if !limits::in_coord_bounds(ha.x) || !limits::in_coord_bounds(ha.y) {
                            return false;
                        }
                        if !limits::in_coord_bounds(hb.x) || !limits::in_coord_bounds(hb.y) {
                            return false;
                        }
                    }
                    EdgeDeKind::Polyline { points } => {
                        if points.len() > limits::MAX_POLYLINE_POINTS_PER_EDGE {
                            return false;
                        }
                        poly_total += points.len();
                        if poly_total > limits::MAX_POLYLINE_POINTS_TOTAL {
                            return false;
                        }
                        for p in points {
                            if !limits::in_coord_bounds(p.x) || !limits::in_coord_bounds(p.y) {
                                return false;
                            }
                        }
                    }
                }
            }
            if let Some(w) = e.width {
                if !limits::in_width_bounds(w) {
                    return false;
                }
            }
            if let Some(c) = e.stroke {
                let _ = (c.r, c.g, c.b, c.a);
            }
        }
        let max_node = doc.nodes.iter().map(|n| n.id).max().unwrap_or(0);
        let max_edge = doc.edges.iter().map(|e| e.id).max().unwrap_or(0);
        g.nodes = vec![None; (max_node as usize) + 1];
        g.edges = vec![None; (max_edge as usize) + 1];
        g.fills.clear();

        // Collect edge IDs for layer assignment
        let mut loaded_edge_ids: Vec<u32> = Vec::new();

        for n in doc.nodes {
            if !limits::in_coord_bounds(n.x) || !limits::in_coord_bounds(n.y) {
                return false;
            }
            g.nodes[n.id as usize] = Some(crate::model::Node { x: n.x, y: n.y });
        }
        for e in doc.edges {
            let a_ok = g.nodes.get(e.a as usize).and_then(|n| *n).is_some();
            let b_ok = g.nodes.get(e.b as usize).and_then(|n| *n).is_some();
            if !a_ok || !b_ok {
                continue;
            }
            let kind = match e.kind.unwrap_or(EdgeDeKind::Line) {
                EdgeDeKind::Line => crate::model::EdgeKind::Line,
                EdgeDeKind::Cubic { ha, hb, mode } => crate::model::EdgeKind::Cubic {
                    ha,
                    hb,
                    mode: mode.unwrap_or(HandleMode::Free),
                },
                EdgeDeKind::Polyline { points } => crate::model::EdgeKind::Polyline { points },
            };
            let width = e.width.unwrap_or(2.0);
            if !limits::in_width_bounds(width) {
                return false;
            }
            g.edges[e.id as usize] = Some(crate::model::Edge {
                a: e.a,
                b: e.b,
                kind,
                stroke: e.stroke,
                stroke_width: width,
            });
            loaded_edge_ids.push(e.id);
        }
        if let Some(fills) = doc.fills {
            for f in fills {
                g.fills.insert(
                    f.key,
                    FillState {
                        filled: f.filled,
                        color: f.color,
                    },
                );
            }
        }

        // Load layers and groups if present (v2 format), otherwise migrate v1
        if let (Some(layers), Some(groups)) = (doc.layers, doc.groups) {
            // V2 format: restore layer system
            let mut layer_system = LayerSystem::default();

            // Find the max ID to set next_id properly
            let max_layer_id = layers.iter().map(|l| l.id).max().unwrap_or(0);
            let max_group_id = groups.iter().map(|gr| gr.id).max().unwrap_or(0);
            layer_system.next_id = max_layer_id.max(max_group_id) + 1;

            // Restore layers
            for l in layers {
                layer_system.layers.push(Layer {
                    id: l.id,
                    name: l.name,
                    z_index: l.z_index,
                    visible: l.visible,
                    locked: l.locked,
                    opacity: l.opacity,
                    root_group: l.root_group,
                });
            }

            // Restore groups
            for gr in groups {
                layer_system.groups.insert(
                    gr.id,
                    Group {
                        id: gr.id,
                        name: gr.name,
                        parent: gr.parent,
                        children: gr.children,
                        edges: gr.edges.clone(),
                        visible: gr.visible,
                        locked: gr.locked,
                        opacity: gr.opacity,
                    },
                );
                // Rebuild edge_to_group mapping
                for eid in gr.edges {
                    layer_system.edge_to_group.insert(eid, gr.id);
                }
            }

            g.layer_system = layer_system;
        } else {
            // V1 format: create default layer and assign all edges to it
            g.layer_system = LayerSystem::new();
            if let Some(default_group) = g.layer_system.default_group() {
                for eid in loaded_edge_ids {
                    g.layer_system.add_edge_to_group(eid, default_group);
                }
            }
        }

        // Load gradients if present
        g.gradients.clear();
        if let Some(gradients) = doc.gradients {
            let max_gradient_id = gradients.iter().map(|gr| gr.id).max().unwrap_or(0);
            g.next_gradient_id = max_gradient_id + 1;
            for gr in gradients {
                g.gradients.insert(gr.id, gr.gradient);
            }
        } else {
            g.next_gradient_id = 0;
        }

        // Load texts if present (v3 format)
        g.texts.clear();
        if let Some(texts) = doc.texts {
            let max_text_id = texts.iter().map(|t| t.id).max().unwrap_or(0);
            g.texts = vec![None; (max_text_id as usize) + 1];
            for t in texts {
                let idx = t.id as usize;
                if idx < g.texts.len() {
                    g.texts[idx] = Some(t);
                }
            }
        }

        // Load effects if present (v4 format)
        g.effects.clear();
        g.shape_effects.clear();
        g.region_effects.clear();
        g.text_effects.clear();
        g.group_effects.clear();
        if let Some(effects) = doc.effects {
            let max_effect_id = effects.iter().map(|e| e.id).max().unwrap_or(0);
            g.next_effect_id = max_effect_id + 1;
            for e in effects {
                g.effects.insert(e.id, e.effect);
            }
        } else {
            g.next_effect_id = 0;
        }
        if let Some(bindings) = doc.effect_bindings {
            for b in bindings {
                let stack = EffectStack {
                    effects: b.effects,
                    enabled: b.enabled,
                };
                match b.target_type.as_str() {
                    "shape" => { g.shape_effects.insert(b.target_id, stack); }
                    "region" => { g.region_effects.insert(b.target_id, stack); }
                    "text" => { g.text_effects.insert(b.target_id, stack); }
                    "group" => { g.group_effects.insert(b.target_id, stack); }
                    _ => {}
                }
            }
        }

        g.geom_ver = g.geom_ver.wrapping_add(1);
        true
    } else {
        false
    }
}

// Strict variant: returns rich error codes instead of boolean
pub fn from_json_impl_strict(g: &mut Graph, v: Value) -> Result<bool, (&'static str, String)> {
    #[derive(Deserialize)]
    struct NodeDe {
        id: u32,
        x: f32,
        y: f32,
    }
    #[derive(Deserialize)]
    #[serde(tag = "kind", rename_all = "lowercase")]
    enum EdgeDeKind {
        Line,
        Cubic {
            ha: Vec2,
            hb: Vec2,
            mode: Option<HandleMode>,
        },
        Polyline {
            points: Vec<Vec2>,
        },
    }
    #[derive(Deserialize)]
    struct EdgeDe {
        id: u32,
        a: u32,
        b: u32,
        #[serde(flatten)]
        kind: Option<EdgeDeKind>,
        stroke: Option<Color>,
        width: Option<f32>,
    }
    #[derive(Deserialize)]
    struct FillDe {
        key: u32,
        filled: bool,
        color: Option<Color>,
    }
    #[derive(Deserialize)]
    struct LayerDe {
        id: LayerId,
        name: String,
        z_index: i32,
        visible: bool,
        locked: bool,
        opacity: f32,
        root_group: LayerId,
    }
    #[derive(Deserialize)]
    struct GroupDe {
        id: LayerId,
        name: String,
        parent: Option<LayerId>,
        children: Vec<LayerId>,
        edges: Vec<u32>,
        visible: bool,
        locked: bool,
        opacity: f32,
    }
    #[derive(Deserialize)]
    struct GradientDe {
        id: GradientId,
        #[serde(flatten)]
        gradient: Gradient,
    }
    #[derive(Deserialize)]
    struct EffectDe {
        id: EffectId,
        #[serde(flatten)]
        effect: Effect,
    }
    #[derive(Deserialize)]
    struct EffectBindingDe {
        target_type: String,
        target_id: u32,
        effects: Vec<EffectId>,
        enabled: bool,
    }
    #[derive(Deserialize)]
    struct DocDe {
        version: Option<u32>,
        nodes: Vec<NodeDe>,
        edges: Vec<EdgeDe>,
        fills: Option<Vec<FillDe>>,
        layers: Option<Vec<LayerDe>>,
        groups: Option<Vec<GroupDe>>,
        gradients: Option<Vec<GradientDe>>,
        texts: Option<Vec<TextElement>>,
        effects: Option<Vec<EffectDe>>,
        effect_bindings: Option<Vec<EffectBindingDe>>,
    }
    let doc: DocDe = serde_json::from_value(v).map_err(|e| ("json_parse", format!("{}", e)))?;
    if doc.nodes.len() > limits::MAX_NODES {
        return Err(("caps_exceeded", format!("nodes>{}", limits::MAX_NODES)));
    }
    if doc.edges.len() > limits::MAX_EDGES {
        return Err(("caps_exceeded", format!("edges>{}", limits::MAX_EDGES)));
    }
    for n in &doc.nodes {
        if !limits::in_coord_bounds(n.x) || !limits::in_coord_bounds(n.y) {
            return Err(("out_of_bounds", "node coordinate".into()));
        }
    }
    let mut poly_total: usize = 0;
    for e in &doc.edges {
        if e.a == e.b {
            return Err(("invalid_structure", "edge endpoints equal".into()));
        }
        if let Some(kind) = &e.kind {
            match kind {
                EdgeDeKind::Line => {}
                EdgeDeKind::Cubic { ha, hb, .. } => {
                    if !limits::in_coord_bounds(ha.x) || !limits::in_coord_bounds(ha.y) {
                        return Err(("out_of_bounds", "ha".into()));
                    }
                    if !limits::in_coord_bounds(hb.x) || !limits::in_coord_bounds(hb.y) {
                        return Err(("out_of_bounds", "hb".into()));
                    }
                }
                EdgeDeKind::Polyline { points } => {
                    if points.len() > limits::MAX_POLYLINE_POINTS_PER_EDGE {
                        return Err((
                            "caps_exceeded",
                            format!(
                                "polyline_points_per_edge>{}",
                                limits::MAX_POLYLINE_POINTS_PER_EDGE
                            ),
                        ));
                    }
                    poly_total += points.len();
                    if poly_total > limits::MAX_POLYLINE_POINTS_TOTAL {
                        return Err((
                            "caps_exceeded",
                            format!(
                                "polyline_points_total>{}",
                                limits::MAX_POLYLINE_POINTS_TOTAL
                            ),
                        ));
                    }
                    for p in points {
                        if !limits::in_coord_bounds(p.x) || !limits::in_coord_bounds(p.y) {
                            return Err(("out_of_bounds", "polyline point".into()));
                        }
                    }
                }
            }
        }
        if let Some(w) = e.width {
            if !limits::in_width_bounds(w) {
                return Err(("out_of_bounds", "width".into()));
            }
        }
    }
    let max_node = doc.nodes.iter().map(|n| n.id).max().unwrap_or(0);
    let max_edge = doc.edges.iter().map(|e| e.id).max().unwrap_or(0);
    g.nodes = vec![None; (max_node as usize) + 1];
    g.edges = vec![None; (max_edge as usize) + 1];
    g.fills.clear();

    // Collect edge IDs for layer assignment
    let mut loaded_edge_ids: Vec<u32> = Vec::new();

    for n in doc.nodes {
        if !limits::in_coord_bounds(n.x) || !limits::in_coord_bounds(n.y) {
            return Err(("out_of_bounds", "node coordinate".into()));
        }
        g.nodes[n.id as usize] = Some(crate::model::Node { x: n.x, y: n.y });
    }
    for e in doc.edges {
        let a_ok = g.nodes.get(e.a as usize).and_then(|n| *n).is_some();
        let b_ok = g.nodes.get(e.b as usize).and_then(|n| *n).is_some();
        if !a_ok || !b_ok {
            continue;
        }
        let kind = match e.kind.unwrap_or(EdgeDeKind::Line) {
            EdgeDeKind::Line => crate::model::EdgeKind::Line,
            EdgeDeKind::Cubic { ha, hb, mode } => crate::model::EdgeKind::Cubic {
                ha,
                hb,
                mode: mode.unwrap_or(HandleMode::Free),
            },
            EdgeDeKind::Polyline { points } => crate::model::EdgeKind::Polyline { points },
        };
        let width = e.width.unwrap_or(2.0);
        if !limits::in_width_bounds(width) {
            return Err(("out_of_bounds", "width".into()));
        }
        g.edges[e.id as usize] = Some(crate::model::Edge {
            a: e.a,
            b: e.b,
            kind,
            stroke: e.stroke,
            stroke_width: width,
        });
        loaded_edge_ids.push(e.id);
    }
    if let Some(fills) = doc.fills {
        for f in fills {
            g.fills.insert(
                f.key,
                FillState {
                    filled: f.filled,
                    color: f.color,
                },
            );
        }
    }

    // Load layers and groups if present (v2 format), otherwise migrate v1
    if let (Some(layers), Some(groups)) = (doc.layers, doc.groups) {
        // V2 format: restore layer system
        let mut layer_system = LayerSystem::default();

        // Find the max ID to set next_id properly
        let max_layer_id = layers.iter().map(|l| l.id).max().unwrap_or(0);
        let max_group_id = groups.iter().map(|gr| gr.id).max().unwrap_or(0);
        layer_system.next_id = max_layer_id.max(max_group_id) + 1;

        // Restore layers
        for l in layers {
            layer_system.layers.push(Layer {
                id: l.id,
                name: l.name,
                z_index: l.z_index,
                visible: l.visible,
                locked: l.locked,
                opacity: l.opacity,
                root_group: l.root_group,
            });
        }

        // Restore groups
        for gr in groups {
            layer_system.groups.insert(
                gr.id,
                Group {
                    id: gr.id,
                    name: gr.name,
                    parent: gr.parent,
                    children: gr.children,
                    edges: gr.edges.clone(),
                    visible: gr.visible,
                    locked: gr.locked,
                    opacity: gr.opacity,
                },
            );
            // Rebuild edge_to_group mapping
            for eid in gr.edges {
                layer_system.edge_to_group.insert(eid, gr.id);
            }
        }

        g.layer_system = layer_system;
    } else {
        // V1 format: create default layer and assign all edges to it
        g.layer_system = LayerSystem::new();
        if let Some(default_group) = g.layer_system.default_group() {
            for eid in loaded_edge_ids {
                g.layer_system.add_edge_to_group(eid, default_group);
            }
        }
    }

    // Load gradients if present
    g.gradients.clear();
    if let Some(gradients) = doc.gradients {
        let max_gradient_id = gradients.iter().map(|gr| gr.id).max().unwrap_or(0);
        g.next_gradient_id = max_gradient_id + 1;
        for gr in gradients {
            g.gradients.insert(gr.id, gr.gradient);
        }
    } else {
        g.next_gradient_id = 0;
    }

    // Load texts if present (v3 format)
    g.texts.clear();
    if let Some(texts) = doc.texts {
        let max_text_id = texts.iter().map(|t| t.id).max().unwrap_or(0);
        g.texts = vec![None; (max_text_id as usize) + 1];
        for t in texts {
            let idx = t.id as usize;
            if idx < g.texts.len() {
                g.texts[idx] = Some(t);
            }
        }
    }

    // Load effects if present (v4 format)
    g.effects.clear();
    g.shape_effects.clear();
    g.region_effects.clear();
    g.text_effects.clear();
    g.group_effects.clear();
    if let Some(effects) = doc.effects {
        let max_effect_id = effects.iter().map(|e| e.id).max().unwrap_or(0);
        g.next_effect_id = max_effect_id + 1;
        for e in effects {
            g.effects.insert(e.id, e.effect);
        }
    } else {
        g.next_effect_id = 0;
    }
    if let Some(bindings) = doc.effect_bindings {
        for b in bindings {
            let stack = EffectStack {
                effects: b.effects,
                enabled: b.enabled,
            };
            match b.target_type.as_str() {
                "shape" => { g.shape_effects.insert(b.target_id, stack); }
                "region" => { g.region_effects.insert(b.target_id, stack); }
                "text" => { g.text_effects.insert(b.target_id, stack); }
                "group" => { g.group_effects.insert(b.target_id, stack); }
                _ => {}
            }
        }
    }

    g.geom_ver = g.geom_ver.wrapping_add(1);
    Ok(true)
}
