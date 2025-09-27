use crate::geometry::limits;
use crate::{
    model::{Color, FillState, HandleMode, Vec2},
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
    struct Doc {
        version: u32,
        nodes: Vec<NodeSer>,
        edges: Vec<EdgeSer>,
        fills: Vec<FillSer>,
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
    serde_json::to_value(Doc {
        version: 1,
        nodes,
        edges,
        fills,
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
    struct DocDe {
        version: Option<u32>,
        nodes: Vec<NodeDe>,
        edges: Vec<EdgeDe>,
        fills: Option<Vec<FillDe>>,
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
    struct DocDe {
        version: Option<u32>,
        nodes: Vec<NodeDe>,
        edges: Vec<EdgeDe>,
        fills: Option<Vec<FillDe>>,
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
    g.geom_ver = g.geom_ver.wrapping_add(1);
    Ok(true)
}
