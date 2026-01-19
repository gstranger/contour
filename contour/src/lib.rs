pub mod model;
pub mod layers;
pub mod geometry {
    pub mod cubic;
    pub mod flatten;
    pub mod intersect;
    pub mod limits;
    pub mod math;
    pub mod path_length;
    pub mod tolerance;
}
pub mod algorithms {
    pub mod boolean;
    pub mod incremental;
    pub mod picking;
    pub mod planarize;
    pub mod planarize_subset;
    pub mod regions;
    pub mod text_layout;
    pub mod text_outline;
    pub mod winding;
}
mod json;
mod svg;

use layers::LayerSystem;
use model::{
    Color, ColorStop, Edge, EdgeKind, FillRule, FillState, FontStyle, Gradient, GradientId,
    GradientUnits, HandleMode, LayerId, LinearGradient, Node, Paint, PrimitiveResult, RadialGradient,
    Shape, SpreadMethod, TextAlign, TextElement, TextId, TextStyle, TextType, Vec2, VerticalAlign,
    TextOverflow,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct DirtyState {
    pub since_ver: u64,
    pub nodes_added: HashSet<u32>,
    pub nodes_removed: HashSet<u32>,
    pub nodes_moved: HashSet<u32>,
    pub edges_added: HashSet<u32>,
    pub edges_removed: HashSet<u32>,
    pub edges_modified: HashSet<u32>,
    pub bbox: Option<(f32, f32, f32, f32)>, // minx,miny,maxx,maxy
    pub full: bool,
}

#[derive(Clone, Debug)]
pub struct RegionFaceCache {
    pub key: u32,
    pub area: f32,
    pub bbox: (f32, f32, f32, f32),
    pub points: Vec<Vec2>,
    pub edges: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct RegionCache {
    pub faces: Vec<RegionFaceCache>,
    pub built_ver: u64,
    pub tol: f32,
}

pub struct Graph {
    pub(crate) nodes: Vec<Option<Node>>,       // id is index
    pub(crate) edges: Vec<Option<Edge>>,       // id is index
    pub(crate) shapes: Vec<Option<Shape>>,     // id is index
    pub(crate) texts: Vec<Option<TextElement>>, // id is index
    pub(crate) fills: HashMap<u32, FillState>, // region key -> fill
    pub(crate) layer_system: LayerSystem,      // layer/group hierarchy
    pub(crate) gradients: HashMap<GradientId, Gradient>, // gradient definitions
    pub(crate) next_gradient_id: GradientId,   // next gradient ID
    pub(crate) geom_ver: u64,
    pub(crate) last_geom_ver: u64,
    pub(crate) prev_regions: Vec<(u32, i32, i32, f32)>, // (key, qcx, qcy, area)
    pub(crate) flatten_tol: f32,
    // Picking spatial index: (built_geom_ver, index)
    pub(crate) pick_index: RefCell<Option<(u64, crate::algorithms::picking::PickIndex)>>,
    // Incremental regions bookkeeping
    pub(crate) dirty: DirtyState,
    pub(crate) region_cache: RefCell<Option<RegionCache>>,
    pub(crate) flatten_index: RefCell<Option<crate::algorithms::regions::FlattenIndex>>,
    pub(crate) flatten_cache: RefCell<Option<crate::algorithms::regions::FlattenCache>>,
    pub(crate) incr_plan: RefCell<Option<crate::algorithms::incremental::IncrPlan>>,
}

pub struct EdgeArrays {
    pub ids: Vec<u32>,
    pub endpoints: Vec<u32>,
    pub kinds: Vec<u8>,
    pub stroke_rgba: Vec<u8>,
    pub stroke_widths: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
pub enum Pick {
    #[serde(rename = "node")]
    Node { id: u32, dist: f32 },
    #[serde(rename = "edge")]
    Edge { id: u32, t: f32, dist: f32 },
    #[serde(rename = "handle")]
    Handle { edge: u32, end: u8, dist: f32 },
}

impl Graph {
    // Enforce handle constraints after edits. If changed_end is Some(0|1), we
    // preserve that end's length for Aligned, and mirror the other to equal length for Mirrored.
    fn enforce_handle_constraints(
        ha: Vec2,
        hb: Vec2,
        mode: HandleMode,
        changed_end: Option<u8>,
    ) -> (Vec2, Vec2) {
        use geometry::tolerance::EPS_LEN;
        match mode {
            HandleMode::Free => (ha, hb),
            HandleMode::Mirrored => {
                // Opposite directions; equal lengths.
                let la = (ha.x * ha.x + ha.y * ha.y).sqrt();
                let lb = (hb.x * hb.x + hb.y * hb.y).sqrt();
                if la <= EPS_LEN && lb <= EPS_LEN {
                    return (Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 0.0, y: 0.0 });
                }
                // Choose target length. If a specific end changed, use its length; else average.
                let target_len = match changed_end {
                    Some(0) => la,
                    Some(1) => lb,
                    _ => 0.5 * (la.max(0.0) + lb.max(0.0)),
                };
                // Direction from ha defines both (hb opposite).
                let (ux, uy) = if la > EPS_LEN {
                    (ha.x / la, ha.y / la)
                } else if lb > EPS_LEN {
                    (-hb.x / lb, -hb.y / lb)
                } else {
                    (0.0, 0.0)
                };
                let nha = Vec2 {
                    x: ux * target_len,
                    y: uy * target_len,
                };
                let nhb = Vec2 {
                    x: -ux * target_len,
                    y: -uy * target_len,
                };
                (nha, nhb)
            }
            HandleMode::Aligned => {
                // Opposite directions; preserve opposite length for the changed end if provided.
                let la = (ha.x * ha.x + ha.y * ha.y).sqrt();
                let lb = (hb.x * hb.x + hb.y * hb.y).sqrt();
                // Direction from ha if available, else opposite of hb.
                let (ux, uy, ref_len_b) = if changed_end == Some(0) {
                    let (ux, uy) = if la > EPS_LEN {
                        (ha.x / la, ha.y / la)
                    } else {
                        (0.0, 0.0)
                    };
                    (ux, uy, lb)
                } else if changed_end == Some(1) {
                    // Use hb direction to infer ha; preserve ha length
                    let (vx, vy) = if lb > EPS_LEN {
                        (hb.x / lb, hb.y / lb)
                    } else {
                        (0.0, 0.0)
                    };
                    (-vx, -vy, la)
                } else {
                    // Both changed: align by ha's direction and preserve hb length.
                    let (ux, uy) = if la > EPS_LEN {
                        (ha.x / la, ha.y / la)
                    } else if lb > EPS_LEN {
                        (-hb.x / lb, -hb.y / lb)
                    } else {
                        (0.0, 0.0)
                    };
                    (ux, uy, lb)
                };
                let nha = if la > EPS_LEN {
                    Vec2 {
                        x: ux * la,
                        y: uy * la,
                    }
                } else {
                    ha
                };
                let nhb = if ref_len_b > EPS_LEN {
                    Vec2 {
                        x: -ux * ref_len_b,
                        y: -uy * ref_len_b,
                    }
                } else {
                    Vec2 { x: 0.0, y: 0.0 }
                };
                (nha, nhb)
            }
        }
    }
    pub fn new() -> Self {
        Graph {
            nodes: Vec::new(),
            edges: Vec::new(),
            shapes: Vec::new(),
            texts: Vec::new(),
            fills: HashMap::new(),
            layer_system: LayerSystem::new(),
            gradients: HashMap::new(),
            next_gradient_id: 0,
            geom_ver: 1,
            last_geom_ver: 0,
            prev_regions: Vec::new(),
            flatten_tol: 0.25,
            pick_index: RefCell::new(None),
            dirty: DirtyState {
                since_ver: 1,
                ..Default::default()
            },
            region_cache: RefCell::new(None),
            flatten_index: RefCell::new(None),
            flatten_cache: RefCell::new(None),
            incr_plan: RefCell::new(None),
        }
    }
    pub fn geom_version(&self) -> u64 {
        self.geom_ver
    }

    fn union_bbox(
        a: Option<(f32, f32, f32, f32)>,
        b: Option<(f32, f32, f32, f32)>,
    ) -> Option<(f32, f32, f32, f32)> {
        match (a, b) {
            (None, x) => x,
            (x, None) => x,
            (Some((ax0, ay0, ax1, ay1)), Some((bx0, by0, bx1, by1))) => {
                Some((ax0.min(bx0), ay0.min(by0), ax1.max(bx1), ay1.max(by1)))
            }
        }
    }

    fn expand_dirty_bbox_pt(&mut self, x: f32, y: f32) {
        let p = Some((x, y, x, y));
        self.dirty.bbox = Self::union_bbox(self.dirty.bbox, p);
    }

    fn expand_dirty_bbox_box(&mut self, b: Option<(f32, f32, f32, f32)>) {
        self.dirty.bbox = Self::union_bbox(self.dirty.bbox, b);
    }

    fn expand_dirty_bbox_around(&mut self, x: f32, y: f32, pad: f32) {
        let p = Some((x - pad, y - pad, x + pad, y + pad));
        self.dirty.bbox = Self::union_bbox(self.dirty.bbox, p);
    }

    pub(crate) fn edge_aabb_of(&self, e: &Edge) -> Option<(f32, f32, f32, f32)> {
        let a = self.nodes.get(e.a as usize).and_then(|n| *n)?;
        let b = self.nodes.get(e.b as usize).and_then(|n| *n)?;
        match &e.kind {
            EdgeKind::Line => {
                let minx = a.x.min(b.x);
                let maxx = a.x.max(b.x);
                let miny = a.y.min(b.y);
                let maxy = a.y.max(b.y);
                Some((minx, miny, maxx, maxy))
            }
            EdgeKind::Cubic { ha, hb, .. } => {
                let p1x = a.x + ha.x;
                let p1y = a.y + ha.y;
                let p2x = b.x + hb.x;
                let p2y = b.y + hb.y;
                let minx = a.x.min(b.x).min(p1x).min(p2x);
                let maxx = a.x.max(b.x).max(p1x).max(p2x);
                let miny = a.y.min(b.y).min(p1y).min(p2y);
                let maxy = a.y.max(b.y).max(p1y).max(p2y);
                Some((minx, miny, maxx, maxy))
            }
            EdgeKind::Polyline { points } => {
                let mut minx = a.x.min(b.x);
                let mut maxx = a.x.max(b.x);
                let mut miny = a.y.min(b.y);
                let mut maxy = a.y.max(b.y);
                for p in points {
                    minx = minx.min(p.x);
                    maxx = maxx.max(p.x);
                    miny = miny.min(p.y);
                    maxy = maxy.max(p.y);
                }
                Some((minx, miny, maxx, maxy))
            }
        }
    }

    fn mark_edge_endpoints_dirty(&mut self, eid: u32, pad: f32) {
        if let Some(Some(edge)) = self.edges.get(eid as usize) {
            if let Some(bb) = self.edge_aabb_of(edge) {
                self.expand_dirty_bbox_box(Some((bb.0 - pad, bb.1 - pad, bb.2 + pad, bb.3 + pad)));
            }
            self.dirty.edges_modified.insert(eid);
        }
    }

    pub fn dirty_reset(&mut self) {
        self.dirty = DirtyState {
            since_ver: self.geom_ver,
            ..Default::default()
        };
    }

    pub(crate) fn clear_dirty_flags(&mut self) {
        self.dirty.full = false;
        self.dirty.bbox = None;
        self.dirty.nodes_added.clear();
        self.dirty.nodes_removed.clear();
        self.dirty.nodes_moved.clear();
        self.dirty.edges_added.clear();
        self.dirty.edges_removed.clear();
        self.dirty.edges_modified.clear();
        self.dirty.since_ver = self.geom_ver;
    }

    pub(crate) fn mark_full_dirty(&mut self) {
        self.dirty.full = true;
        self.dirty.bbox = None;
    }

    // Nodes
    pub fn add_node(&mut self, x: f32, y: f32) -> u32 {
        let id = self.nodes.len() as u32;
        self.nodes.push(Some(Node { x, y }));
        self.dirty.nodes_added.insert(id);
        self.expand_dirty_bbox_pt(x, y);
        self.bump();
        id
    }
    pub fn move_node(&mut self, id: u32, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let (oldx, oldy) = match self.nodes.get(id as usize).and_then(|n| *n) {
            Some(n) => (n.x, n.y),
            None => return false,
        };
        let dx = x - oldx;
        let dy = y - oldy;
        if (dx * dx + dy * dy)
            <= crate::geometry::tolerance::EPS_POS * crate::geometry::tolerance::EPS_POS
        {
            return true;
        }
        if let Some(Some(n)) = self.nodes.get_mut(id as usize) {
            n.x = x;
            n.y = y;
        } else {
            return false;
        }
        self.dirty.nodes_moved.insert(id);
        for (eid, e_opt) in self.edges.iter().enumerate() {
            if let Some(e) = e_opt {
                if e.a == id || e.b == id {
                    self.dirty.edges_modified.insert(eid as u32);
                }
            }
        }
        self.expand_dirty_bbox_around(oldx, oldy, 12.0);
        self.expand_dirty_bbox_around(x, y, 12.0);
        self.bump();
        true
    }
    pub fn get_node(&self, id: u32) -> Option<(f32, f32)> {
        self.nodes
            .get(id as usize)
            .and_then(|n| *n)
            .map(|n| (n.x, n.y))
    }
    pub fn remove_node(&mut self, id: u32) -> bool {
        let (nx, ny) = match self.nodes.get(id as usize).and_then(|n| *n) {
            Some(n) => (n.x, n.y),
            None => return false,
        };
        let mut incident: Vec<usize> = Vec::new();
        for (eid, e) in self.edges.iter().enumerate() {
            if let Some(edge) = e {
                if edge.a == id || edge.b == id {
                    incident.push(eid);
                }
            }
        }
        if let Some(slot) = self.nodes.get_mut(id as usize) {
            *slot = None;
        }
        for eid in incident {
            if let Some(edge) = self.edges.get(eid).and_then(|e| e.as_ref()) {
                if let Some(bb) = self.edge_aabb_of(edge) {
                    self.expand_dirty_bbox_box(Some(bb));
                }
            }
            if let Some(slot) = self.edges.get_mut(eid) {
                *slot = None;
            }
            self.dirty.edges_removed.insert(eid as u32);
        }
        self.expand_dirty_bbox_around(nx, ny, 12.0);
        self.dirty.nodes_removed.insert(id);
        self.bump();
        true
    }
    pub fn node_count(&self) -> u32 {
        self.nodes.iter().filter(|n| n.is_some()).count() as u32
    }

    // Edges
    pub fn add_edge(&mut self, a: u32, b: u32) -> Option<u32> {
        if a == b {
            return None;
        }
        if self
            .nodes
            .get(a as usize)
            .and_then(|n| n.as_ref())
            .is_none()
        {
            return None;
        }
        if self
            .nodes
            .get(b as usize)
            .and_then(|n| n.as_ref())
            .is_none()
        {
            return None;
        }
        let id = self.edges.len() as u32;
        self.edges.push(Some(Edge {
            a,
            b,
            kind: EdgeKind::Line,
            stroke: None,
            stroke_width: 2.0,
        }));
        // Assign to default layer's root group
        if let Some(default_group) = self.layer_system.default_group() {
            self.layer_system.add_edge_to_group(id, default_group);
        }
        self.dirty.edges_added.insert(id);
        if let (Some(na), Some(nb)) = (
            self.nodes.get(a as usize).and_then(|n| *n),
            self.nodes.get(b as usize).and_then(|n| *n),
        ) {
            let minx = na.x.min(nb.x);
            let miny = na.y.min(nb.y);
            let maxx = na.x.max(nb.x);
            let maxy = na.y.max(nb.y);
            self.expand_dirty_bbox_box(Some((minx, miny, maxx, maxy)));
        }
        self.bump();
        Some(id)
    }
    pub fn remove_edge(&mut self, id: u32) -> bool {
        let old_bb = if let Some(Some(edge)) = self.edges.get(id as usize) {
            self.edge_aabb_of(edge)
        } else {
            None
        };
        if let Some(slot) = self.edges.get_mut(id as usize) {
            if slot.is_some() {
                *slot = None;
                // Remove from layer system
                self.layer_system.remove_edge(id);
                if let Some(bb) = old_bb {
                    self.expand_dirty_bbox_box(Some(bb));
                }
                self.dirty.edges_removed.insert(id);
                self.bump();
                return true;
            }
        }
        false
    }
    pub fn edge_count(&self) -> u32 {
        self.edges.iter().filter(|e| e.is_some()).count() as u32
    }

    pub fn get_node_arrays(&self) -> (Vec<u32>, Vec<f32>) {
        let mut ids = Vec::new();
        let mut pos = Vec::new();
        for (i, n) in self.nodes.iter().enumerate() {
            if let Some(n) = n {
                ids.push(i as u32);
                pos.push(n.x);
                pos.push(n.y);
            }
        }
        (ids, pos)
    }
    pub fn get_edge_arrays(&self) -> EdgeArrays {
        let mut ids = Vec::new();
        let mut ep = Vec::new();
        let mut kinds = Vec::new();
        let mut rgba = Vec::new();
        let mut widths = Vec::new();
        for (i, e) in self.edges.iter().enumerate() {
            if let Some(e) = e {
                ids.push(i as u32);
                ep.push(e.a);
                ep.push(e.b);
                kinds.push(match e.kind {
                    EdgeKind::Line => 0,
                    EdgeKind::Cubic { .. } => 1,
                    EdgeKind::Polyline { .. } => 2,
                });
                if let Some(c) = e.stroke {
                    rgba.extend_from_slice(&[c.r, c.g, c.b, c.a]);
                    widths.push(e.stroke_width);
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                    widths.push(0.0);
                }
            }
        }
        EdgeArrays {
            ids,
            endpoints: ep,
            kinds,
            stroke_rgba: rgba,
            stroke_widths: widths,
        }
    }

    // Picking return
    pub fn pick(&self, x: f32, y: f32, tol: f32) -> Option<Pick> {
        algorithms::picking::pick_impl(self, x, y, tol)
    }

    // JSON
    pub fn to_json_value(&self) -> serde_json::Value {
        json::to_json_impl(self)
    }
    pub fn from_json_value(&mut self, v: serde_json::Value) -> bool {
        json::from_json_impl(self, v)
    }
    pub fn from_json_value_strict(
        &mut self,
        v: serde_json::Value,
    ) -> Result<bool, (&'static str, String)> {
        json::from_json_impl_strict(self, v)
    }

    // Clear
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.edges.clear();
        self.shapes.clear();
        self.texts.clear();
        self.fills.clear();
        self.prev_regions.clear();
        self.region_cache.borrow_mut().take();
        self.flatten_index.borrow_mut().take();
        self.flatten_cache.borrow_mut().take();
        self.incr_plan.borrow_mut().take();
        self.mark_full_dirty();
        self.bump();
    }

    // Styles and handles
    pub fn set_edge_style(&mut self, id: u32, r: u8, g: u8, b: u8, a: u8, width: f32) -> bool {
        if let Some(Some(e)) = self.edges.get_mut(id as usize) {
            e.stroke = Some(Color { r, g, b, a });
            e.stroke_width = if width > 0.0 { width } else { 2.0 };
            return true;
        }
        false
    }
    pub fn get_edge_style(&self, id: u32) -> Option<(u8, u8, u8, u8, f32)> {
        if let Some(Some(e)) = self.edges.get(id as usize) {
            if let Some(c) = e.stroke {
                return Some((c.r, c.g, c.b, c.a, e.stroke_width));
            }
        }
        None
    }
    // set_edge_cubic defined below with guards
    pub fn set_edge_line(&mut self, id: u32) -> bool {
        let changed = if let Some(Some(edge)) = self.edges.get_mut(id as usize) {
            if matches!(edge.kind, EdgeKind::Line) {
                false
            } else {
                edge.kind = EdgeKind::Line;
                true
            }
        } else {
            return false;
        };
        if changed {
            self.mark_edge_endpoints_dirty(id, 12.0);
            self.bump();
        }
        true
    }
    pub fn get_handles(&self, id: u32) -> Option<[f32; 4]> {
        if let Some(Some(e)) = self.edges.get(id as usize) {
            let a = match self.nodes.get(e.a as usize).and_then(|n| *n) {
                Some(n) => n,
                None => return None,
            };
            let b = match self.nodes.get(e.b as usize).and_then(|n| *n) {
                Some(n) => n,
                None => return None,
            };
            if let EdgeKind::Cubic { ha, hb, .. } = e.kind {
                return Some([a.x + ha.x, a.y + ha.y, b.x + hb.x, b.y + hb.y]);
            }
        }
        None
    }
    pub fn get_handle_mode(&self, id: u32) -> Option<u8> {
        if let Some(Some(e)) = self.edges.get(id as usize) {
            if let EdgeKind::Cubic { mode, .. } = e.kind {
                return Some(match mode {
                    HandleMode::Free => 0,
                    HandleMode::Mirrored => 1,
                    HandleMode::Aligned => 2,
                });
            }
        }
        None
    }
    pub fn set_handle_pos(&mut self, id: u32, end: u8, x: f32, y: f32) -> bool {
        if end != 0 && end != 1 {
            return false;
        }
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let changed = {
            let edge = match self.edges.get_mut(id as usize) {
                Some(Some(edge)) => edge,
                _ => return false,
            };
            let (mut ha, mut hb, mode) = match edge.kind {
                EdgeKind::Cubic { ha, hb, mode } => (ha, hb, mode),
                _ => return false,
            };
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) {
                Some(n) => n,
                None => return false,
            };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) {
                Some(n) => n,
                None => return false,
            };
            if end == 0 {
                let nx = x - a.x;
                let ny = y - a.y;
                let dx = nx - ha.x;
                let dy = ny - ha.y;
                if (dx * dx + dy * dy)
                    <= crate::geometry::tolerance::EPS_POS * crate::geometry::tolerance::EPS_POS
                {
                    return true;
                }
                ha = Vec2 { x: nx, y: ny };
            } else {
                let nx = x - b.x;
                let ny = y - b.y;
                let dx = nx - hb.x;
                let dy = ny - hb.y;
                if (dx * dx + dy * dy)
                    <= crate::geometry::tolerance::EPS_POS * crate::geometry::tolerance::EPS_POS
                {
                    return true;
                }
                hb = Vec2 { x: nx, y: ny };
            }
            let (ha, hb) = Self::enforce_handle_constraints(ha, hb, mode, Some(end));
            edge.kind = EdgeKind::Cubic { ha, hb, mode };
            true
        };
        if changed {
            self.mark_edge_endpoints_dirty(id, 12.0);
            self.bump();
        }
        true
    }
    pub fn set_handle_mode(&mut self, id: u32, mode: u8) -> bool {
        let changed = {
            let edge = match self.edges.get_mut(id as usize) {
                Some(Some(edge)) => edge,
                _ => return false,
            };
            let (ha, hb, cur_mode) = match edge.kind {
                EdgeKind::Cubic { ha, hb, mode } => (ha, hb, mode),
                _ => return false,
            };
            let m = match mode {
                1 => HandleMode::Mirrored,
                2 => HandleMode::Aligned,
                _ => HandleMode::Free,
            };
            if cur_mode as u8 == mode {
                return true;
            }
            let (ha, hb) = Self::enforce_handle_constraints(ha, hb, m, None);
            edge.kind = EdgeKind::Cubic { ha, hb, mode: m };
            true
        };
        if changed {
            self.mark_edge_endpoints_dirty(id, 12.0);
            self.bump();
        }
        true
    }
    pub fn set_edge_cubic(&mut self, id: u32, p1x: f32, p1y: f32, p2x: f32, p2y: f32) -> bool {
        if !p1x.is_finite() || !p1y.is_finite() || !p2x.is_finite() || !p2y.is_finite() {
            return false;
        }
        let changed = {
            let edge = match self.edges.get_mut(id as usize) {
                Some(Some(edge)) => edge,
                _ => return false,
            };
            let a = match self.nodes.get(edge.a as usize).and_then(|n| *n) {
                Some(n) => n,
                None => return false,
            };
            let b = match self.nodes.get(edge.b as usize).and_then(|n| *n) {
                Some(n) => n,
                None => return false,
            };
            let ha = Vec2 {
                x: p1x - a.x,
                y: p1y - a.y,
            };
            let hb = Vec2 {
                x: p2x - b.x,
                y: p2y - b.y,
            };
            let ha_l = (ha.x * ha.x + ha.y * ha.y).sqrt();
            let hb_l = (hb.x * hb.x + hb.y * hb.y).sqrt();
            if ha_l <= geometry::tolerance::EPS_LEN && hb_l <= geometry::tolerance::EPS_LEN {
                if !matches!(edge.kind, EdgeKind::Line) {
                    edge.kind = EdgeKind::Line;
                    true
                } else {
                    false
                }
            } else {
                let mut needs_update = true;
                if let EdgeKind::Cubic {
                    ha: oha, hb: ohb, ..
                } = edge.kind
                {
                    let da = (ha.x - oha.x) * (ha.x - oha.x) + (ha.y - oha.y) * (ha.y - oha.y);
                    let db = (hb.x - ohb.x) * (hb.x - ohb.x) + (hb.y - ohb.y) * (hb.y - ohb.y);
                    if da <= geometry::tolerance::EPS_POS * geometry::tolerance::EPS_POS
                        && db <= geometry::tolerance::EPS_POS * geometry::tolerance::EPS_POS
                    {
                        needs_update = false;
                    }
                }
                if needs_update {
                    edge.kind = EdgeKind::Cubic {
                        ha,
                        hb,
                        mode: HandleMode::Free,
                    };
                    true
                } else {
                    false
                }
            }
        };
        if changed {
            self.mark_edge_endpoints_dirty(id, 12.0);
            self.bump();
        }
        true
    }
    pub fn bend_edge_to(&mut self, id: u32, t: f32, tx: f32, ty: f32, stiffness: f32) -> bool {
        let did_change = {
            let edge = match self.edges.get_mut(id as usize) {
                Some(Some(edge)) => edge,
                _ => return false,
            };
            let a = if let Some(n) = self.nodes.get(edge.a as usize).and_then(|n| *n) {
                n
            } else {
                return true;
            };
            let b = if let Some(n) = self.nodes.get(edge.b as usize).and_then(|n| *n) {
                n
            } else {
                return true;
            };
            let t = geometry::tolerance::clamp01(t);
            let (mut ha, mut hb, mode) = match edge.kind {
                EdgeKind::Cubic { ha, hb, mode } => (ha, hb, mode),
                EdgeKind::Line => {
                    // Convert to a simple cubic aligned with the segment unless degenerate.
                    let dx = b.x - a.x;
                    let dy = b.y - a.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < geometry::tolerance::EPS_LEN {
                        return true;
                    } // no-op on zero-length
                    let k = 0.3 * len;
                    let ux = dx / len;
                    let uy = dy / len;
                    (
                        Vec2 {
                            x: ux * k,
                            y: uy * k,
                        },
                        Vec2 {
                            x: -ux * k,
                            y: -uy * k,
                        },
                        HandleMode::Free,
                    )
                }
                EdgeKind::Polyline { .. } => return false,
            };
            let orig_ha = ha;
            let orig_hb = hb;
            let p1x = a.x + ha.x;
            let p1y = a.y + ha.y;
            let p2x = b.x + hb.x;
            let p2y = b.y + hb.y;
            let (cx, cy) = geometry::math::cubic_point(t, a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y);
            let dx = tx - cx;
            let dy = ty - cy;
            let c1 = 3.0 * (1.0 - t).powi(2) * t;
            let c2 = 3.0 * (1.0 - t) * t.powi(2);
            let l1 = 1.0;
            let l2 = stiffness.max(geometry::tolerance::EPS_LEN);
            let denom = c1 * c1 / l1 + c2 * c2 / l2;
            if denom <= geometry::tolerance::EPS_DENOM {
                // Treat as no-op to avoid instability
                return true;
            }
            let ax = dx / denom;
            let ay = dy / denom;
            let d1x = (c1 / l1) * ax;
            let d1y = (c1 / l1) * ay;
            let d2x = (c2 / l2) * ax;
            let d2y = (c2 / l2) * ay;
            // Apply LS update then enforce constraints
            ha.x += d1x;
            ha.y += d1y;
            hb.x += d2x;
            hb.y += d2y;
            let changed_end = if t <= 0.5 { Some(0) } else { Some(1) };
            let (ha, hb) = Self::enforce_handle_constraints(ha, hb, mode, changed_end);
            // No-op if handles unchanged within epsilon
            let da =
                (ha.x - orig_ha.x) * (ha.x - orig_ha.x) + (ha.y - orig_ha.y) * (ha.y - orig_ha.y);
            let db =
                (hb.x - orig_hb.x) * (hb.x - orig_hb.x) + (hb.y - orig_hb.y) * (hb.y - orig_hb.y);
            if da <= geometry::tolerance::EPS_POS * geometry::tolerance::EPS_POS
                && db <= geometry::tolerance::EPS_POS * geometry::tolerance::EPS_POS
            {
                return true;
            }
            // Commit cubic kind (including line->cubic conversion)
            edge.kind = EdgeKind::Cubic { ha, hb, mode };
            true
        };
        if did_change {
            self.mark_edge_endpoints_dirty(id, 12.0);
            self.bump();
        }
        true
    }

    // Freehand fitting: convert a sampled polyline into a chain of cubic edges
    pub fn add_freehand(&mut self, points: &[(f32, f32)], close: bool) -> Vec<u32> {
        fn rdp(points: &[(f32, f32)], eps: f32) -> Vec<(f32, f32)> {
            if points.len() <= 2 {
                return points.to_vec();
            }
            let eps2 = eps * eps;
            fn perp_dist2(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
                let (px, py) = p;
                let (x1, y1) = a;
                let (x2, y2) = b;
                let vx = x2 - x1;
                let vy = y2 - y1;
                let wx = px - x1;
                let wy = py - y1;
                let vv = vx * vx + vy * vy;
                if vv == 0.0 {
                    return wx * wx + wy * wy;
                }
                let t = (wx * vx + wy * vy) / vv;
                let t = if t < 0.0 {
                    0.0
                } else if t > 1.0 {
                    1.0
                } else {
                    t
                };
                let sx = x1 + t * vx;
                let sy = y1 + t * vy;
                let dx = px - sx;
                let dy = py - sy;
                dx * dx + dy * dy
            }
            fn rec(slice: &[(f32, f32)], eps2: f32, out: &mut Vec<(f32, f32)>) {
                let n = slice.len();
                if n <= 2 {
                    out.push(slice[0]);
                    return;
                }
                let a = slice[0];
                let b = slice[n - 1];
                let mut idx = 0usize;
                let mut md2 = 0.0f32;
                for i in 1..(n - 1) {
                    let d2 = perp_dist2(slice[i], a, b);
                    if d2 > md2 {
                        md2 = d2;
                        idx = i;
                    }
                }
                if md2 > eps2 {
                    rec(&slice[..=idx], eps2, out);
                    rec(&slice[idx..], eps2, out);
                } else {
                    out.push(a);
                }
            }
            let mut out = Vec::new();
            rec(points, eps2, &mut out);
            out.push(*points.last().unwrap());
            out
        }

        fn angle_between(a: (f32, f32), b: (f32, f32)) -> f32 {
            let (ax, ay) = a;
            let (bx, by) = b;
            let da = (ax * ax + ay * ay).sqrt();
            let db = (bx * bx + by * by).sqrt();
            if da == 0.0 || db == 0.0 {
                return 0.0;
            }
            let mut c = (ax * bx + ay * by) / (da * db);
            if c > 1.0 {
                c = 1.0;
            } else if c < -1.0 {
                c = -1.0;
            }
            c.acos().to_degrees()
        }

        fn resample_even(points: &[(f32, f32)], step: f32, close: bool) -> Vec<(f32, f32)> {
            let n = points.len();
            if n == 0 {
                return Vec::new();
            }
            if n == 1 {
                return points.to_vec();
            }
            let mut out: Vec<(f32, f32)> = Vec::new();
            let mut prev = points[0];
            out.push(prev);
            let mut carry = step;
            let mut seg_iter = 0usize;
            let total_segs = if close { n } else { n - 1 };
            let mut i = 0usize;
            while seg_iter < total_segs {
                let j = if i + 1 < n { i + 1 } else { 0 };
                let (x1, y1) = prev;
                let (x2, y2) = points[j];
                let dx = x2 - prev.0;
                let dy = y2 - prev.1; // from prev to segment end
                let seg_len = (dx * dx + dy * dy).sqrt();
                if seg_len >= carry && carry > 0.0 {
                    let t = carry / seg_len;
                    let nx = x1 + t * (x2 - x1);
                    let ny = y1 + t * (y2 - y1);
                    out.push((nx, ny));
                    prev = (nx, ny);
                    // stay on same segment with reduced remaining length
                    carry = step;
                    continue;
                } else {
                    // move to next segment
                    carry -= seg_len;
                    prev = points[j];
                    i = j;
                    seg_iter += 1;
                }
            }
            if !close {
                if *out.last().unwrap() != *points.last().unwrap() {
                    out.push(*points.last().unwrap());
                }
            }
            out
        }

        let mut pts: Vec<(f32, f32)> = points.iter().copied().collect();
        // Basic guard and sampling sanity
        {
            use crate::geometry::tolerance::EPS_POS;
            pts.dedup_by(|a, b| (a.0 - b.0).abs() < EPS_POS && (a.1 - b.1).abs() < EPS_POS);
        }
        if pts.len() < 2 {
            return Vec::new();
        }
        // Simplify then resample to even spacing (~24 px)
        // Strong simplify first
        let rough = if pts.len() > 4 {
            rdp(&pts, 4.0)
        } else {
            pts.clone()
        };
        // Target a small fixed number of anchors across the whole stroke
        let mut total_len = 0.0f32;
        for i in 0..(rough.len().saturating_sub(1)) {
            let (x1, y1) = rough[i];
            let (x2, y2) = rough[i + 1];
            total_len += ((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)).sqrt();
        }
        if close && rough.len() >= 2 {
            let (x1, y1) = rough[rough.len() - 1];
            let (x2, y2) = rough[0];
            total_len += ((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)).sqrt();
        }
        let target_anchors = if close { 8usize } else { 6usize }; // very sparse by default
        let step = if close {
            if target_anchors > 0 {
                (total_len / (target_anchors as f32)).max(40.0)
            } else {
                total_len.max(40.0)
            }
        } else {
            if target_anchors > 1 {
                (total_len / ((target_anchors - 1) as f32)).max(40.0)
            } else {
                total_len.max(40.0)
            }
        };
        let mut simp = resample_even(&rough, step, close);
        // Uniformly downsample to hard cap desired anchors
        let desired = target_anchors.max(if close { 3 } else { 2 });
        if !close {
            if simp.len() > desired {
                let mut reduced: Vec<(f32, f32)> = Vec::with_capacity(desired);
                for k in 0..desired {
                    let idx = if desired > 1 {
                        ((k as f32) * ((simp.len() - 1) as f32) / ((desired - 1) as f32)).round()
                            as usize
                    } else {
                        0
                    };
                    reduced.push(simp[idx.min(simp.len() - 1)]);
                }
                simp = reduced;
            }
        } else {
            if simp.len() > desired {
                let mut reduced: Vec<(f32, f32)> = Vec::with_capacity(desired);
                for k in 0..desired {
                    let idx = ((k as f32) * ((simp.len()) as f32) / (desired as f32)).round()
                        as usize
                        % simp.len();
                    reduced.push(simp[idx]);
                }
                simp = reduced;
            }
        }
        let n = simp.len();
        if n < 2 {
            return Vec::new();
        }

        // Create nodes
        let mut node_ids: Vec<u32> = Vec::with_capacity(n);
        for &(x, y) in &simp {
            node_ids.push(self.add_node(x, y));
        }

        // Segment-wise Catmull–Rom fit with clamping and cusp detection
        let cusp_deg = 160.0f32; // be more conservative about creating corners
        let clamp_factor = 0.8f32; // allow longer handles for smoother shapes
        let mut created_edges: Vec<u32> = Vec::new();
        let seg_count = if close { n } else { n - 1 };
        for i in 0..seg_count {
            let i0 = i % n;
            let i1 = (i + 1) % n;
            let im1 = if i0 > 0 {
                i0 - 1
            } else {
                if close {
                    n - 1
                } else {
                    i0
                }
            };
            let ip2 = if i1 + 1 < n {
                i1 + 1
            } else {
                if close {
                    (i1 + 1) % n
                } else {
                    i1
                }
            };
            let p0 = simp[im1];
            let p1 = simp[i0];
            let p2 = simp[i1];
            let p3 = simp[ip2];
            // Catmull–Rom tangents
            let t1x = 0.5 * (p2.0 - p0.0);
            let t1y = 0.5 * (p2.1 - p0.1);
            let t2x = 0.5 * (p3.0 - p1.0);
            let t2y = 0.5 * (p3.1 - p1.1);
            // Initial controls
            let mut c1x = p1.0 + t1x / 3.0;
            let mut c1y = p1.1 + t1y / 3.0;
            let mut c2x = p2.0 - t2x / 3.0;
            let mut c2y = p2.1 - t2y / 3.0;
            // Cusp detection at p1 and p2
            let v_in = (p1.0 - p0.0, p1.1 - p0.1);
            let v_out = (p2.0 - p1.0, p2.1 - p1.1);
            if angle_between(v_in, v_out) >= cusp_deg {
                c1x = p1.0;
                c1y = p1.1;
            }
            let v_in2 = (p2.0 - p1.0, p2.1 - p1.1);
            let v_out2 = (p3.0 - p2.0, p3.1 - p2.1);
            if angle_between(v_in2, v_out2) >= cusp_deg {
                c2x = p2.0;
                c2y = p2.1;
            }
            // Clamp to segment length
            let seg_len = ((p2.0 - p1.0) * (p2.0 - p1.0) + (p2.1 - p1.1) * (p2.1 - p1.1)).sqrt();
            let max_len = clamp_factor * seg_len;
            let mut hx = c1x - p1.0;
            let mut hy = c1y - p1.1;
            let hl = (hx * hx + hy * hy).sqrt();
            if hl > max_len && hl > 0.0 {
                hx *= max_len / hl;
                hy *= max_len / hl;
                c1x = p1.0 + hx;
                c1y = p1.1 + hy;
            }
            let mut kx = c2x - p2.0;
            let mut ky = c2y - p2.1;
            let kl = (kx * kx + ky * ky).sqrt();
            if kl > max_len && kl > 0.0 {
                kx *= max_len / kl;
                ky *= max_len / kl;
                c2x = p2.0 + kx;
                c2y = p2.1 + ky;
            }
            // Create edge
            if let Some(eid) = self.add_edge(node_ids[i0], node_ids[i1]) {
                // Set cubic with mirrored handles for smoothness
                if let Some(Some(edge)) = self.edges.get_mut(eid as usize) {
                    let ha = Vec2 {
                        x: c1x - p1.0,
                        y: c1y - p1.1,
                    };
                    let hb = Vec2 {
                        x: c2x - p2.0,
                        y: c2y - p2.1,
                    };
                    edge.kind = EdgeKind::Cubic {
                        ha,
                        hb,
                        mode: HandleMode::Mirrored,
                    };
                    self.bump();
                }
                created_edges.push(eid);
            }
        }
        created_edges
    }

    /// Create a rectangle primitive that decomposes into nodes and edges.
    ///
    /// Arguments:
    /// - `x`, `y`: Top-left corner position
    /// - `w`, `h`: Width and height
    /// - `r`: Corner radius (0 for sharp corners)
    ///
    /// Returns a `PrimitiveResult` containing the created nodes, edges, and shape.
    pub fn add_rectangle(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) -> PrimitiveResult {
        let r = r.abs().min(w.abs() / 2.0).min(h.abs() / 2.0);
        let mut node_ids = Vec::new();
        let mut edge_ids = Vec::new();

        if r <= geometry::tolerance::EPS_LEN {
            // Sharp corners: 4 nodes at corners, 4 line edges
            let n0 = self.add_node(x, y);         // top-left
            let n1 = self.add_node(x + w, y);     // top-right
            let n2 = self.add_node(x + w, y + h); // bottom-right
            let n3 = self.add_node(x, y + h);     // bottom-left
            node_ids = vec![n0, n1, n2, n3];

            // Create edges: top, right, bottom, left
            if let Some(e0) = self.add_edge(n0, n1) { edge_ids.push(e0); }
            if let Some(e1) = self.add_edge(n1, n2) { edge_ids.push(e1); }
            if let Some(e2) = self.add_edge(n2, n3) { edge_ids.push(e2); }
            if let Some(e3) = self.add_edge(n3, n0) { edge_ids.push(e3); }
        } else {
            // Rounded corners: 8 nodes (arc endpoints), 4 line edges + 4 cubic quarter-arcs
            // Kappa for circular arc approximation
            const KAPPA: f32 = 0.5522847498;
            let k = KAPPA * r;

            // Arc endpoint nodes (clockwise from top-left arc end)
            // Top edge: from (x+r, y) to (x+w-r, y)
            let n0 = self.add_node(x + r, y);         // top-left arc end (top side)
            let n1 = self.add_node(x + w - r, y);     // top-right arc start (top side)
            // Right edge: from (x+w, y+r) to (x+w, y+h-r)
            let n2 = self.add_node(x + w, y + r);     // top-right arc end (right side)
            let n3 = self.add_node(x + w, y + h - r); // bottom-right arc start (right side)
            // Bottom edge: from (x+w-r, y+h) to (x+r, y+h)
            let n4 = self.add_node(x + w - r, y + h); // bottom-right arc end (bottom side)
            let n5 = self.add_node(x + r, y + h);     // bottom-left arc start (bottom side)
            // Left edge: from (x, y+h-r) to (x, y+r)
            let n6 = self.add_node(x, y + h - r);     // bottom-left arc end (left side)
            let n7 = self.add_node(x, y + r);         // top-left arc start (left side)

            node_ids = vec![n0, n1, n2, n3, n4, n5, n6, n7];

            // Line edges (straight sides)
            if let Some(e) = self.add_edge(n0, n1) { edge_ids.push(e); } // top
            if let Some(e) = self.add_edge(n2, n3) { edge_ids.push(e); } // right
            if let Some(e) = self.add_edge(n4, n5) { edge_ids.push(e); } // bottom
            if let Some(e) = self.add_edge(n6, n7) { edge_ids.push(e); } // left

            // Corner arcs (cubic bezier approximations)
            // Top-right corner: n1 -> n2
            if let Some(e) = self.add_edge(n1, n2) {
                if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                    edge.kind = EdgeKind::Cubic {
                        ha: Vec2 { x: k, y: 0.0 },
                        hb: Vec2 { x: 0.0, y: -k },
                        mode: HandleMode::Free,
                    };
                }
                edge_ids.push(e);
            }
            // Bottom-right corner: n3 -> n4
            if let Some(e) = self.add_edge(n3, n4) {
                if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                    edge.kind = EdgeKind::Cubic {
                        ha: Vec2 { x: 0.0, y: k },
                        hb: Vec2 { x: k, y: 0.0 },
                        mode: HandleMode::Free,
                    };
                }
                edge_ids.push(e);
            }
            // Bottom-left corner: n5 -> n6
            if let Some(e) = self.add_edge(n5, n6) {
                if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                    edge.kind = EdgeKind::Cubic {
                        ha: Vec2 { x: -k, y: 0.0 },
                        hb: Vec2 { x: 0.0, y: k },
                        mode: HandleMode::Free,
                    };
                }
                edge_ids.push(e);
            }
            // Top-left corner: n7 -> n0
            if let Some(e) = self.add_edge(n7, n0) {
                if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                    edge.kind = EdgeKind::Cubic {
                        ha: Vec2 { x: 0.0, y: -k },
                        hb: Vec2 { x: -k, y: 0.0 },
                        mode: HandleMode::Free,
                    };
                }
                edge_ids.push(e);
            }
        }

        // Create closed shape from edges
        let shape_id = self.create_shape(&edge_ids, true).unwrap_or(0);

        PrimitiveResult {
            nodes: node_ids,
            edges: edge_ids,
            shape: shape_id,
        }
    }

    /// Create an ellipse primitive that decomposes into nodes and edges.
    ///
    /// Arguments:
    /// - `cx`, `cy`: Center position
    /// - `rx`, `ry`: Horizontal and vertical radii
    ///
    /// Returns a `PrimitiveResult` containing the created nodes, edges, and shape.
    pub fn add_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32) -> PrimitiveResult {
        // Standard 4-cubic approximation with kappa
        const KAPPA: f32 = 0.5522847498;
        let kx = KAPPA * rx;
        let ky = KAPPA * ry;

        // 4 nodes at cardinal points (right, top, left, bottom)
        let n_right = self.add_node(cx + rx, cy);      // 0 degrees
        let n_top = self.add_node(cx, cy - ry);        // 90 degrees (up)
        let n_left = self.add_node(cx - rx, cy);       // 180 degrees
        let n_bottom = self.add_node(cx, cy + ry);     // 270 degrees (down)

        let node_ids = vec![n_right, n_top, n_left, n_bottom];
        let mut edge_ids = Vec::new();

        // Quarter 1: right -> top
        if let Some(e) = self.add_edge(n_right, n_top) {
            if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                edge.kind = EdgeKind::Cubic {
                    ha: Vec2 { x: 0.0, y: -ky },   // control from right point going up
                    hb: Vec2 { x: kx, y: 0.0 },    // control from top point going right
                    mode: HandleMode::Free,
                };
            }
            edge_ids.push(e);
        }

        // Quarter 2: top -> left
        if let Some(e) = self.add_edge(n_top, n_left) {
            if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                edge.kind = EdgeKind::Cubic {
                    ha: Vec2 { x: -kx, y: 0.0 },   // control from top point going left
                    hb: Vec2 { x: 0.0, y: -ky },   // control from left point going up
                    mode: HandleMode::Free,
                };
            }
            edge_ids.push(e);
        }

        // Quarter 3: left -> bottom
        if let Some(e) = self.add_edge(n_left, n_bottom) {
            if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                edge.kind = EdgeKind::Cubic {
                    ha: Vec2 { x: 0.0, y: ky },    // control from left point going down
                    hb: Vec2 { x: -kx, y: 0.0 },   // control from bottom point going left
                    mode: HandleMode::Free,
                };
            }
            edge_ids.push(e);
        }

        // Quarter 4: bottom -> right
        if let Some(e) = self.add_edge(n_bottom, n_right) {
            if let Some(Some(edge)) = self.edges.get_mut(e as usize) {
                edge.kind = EdgeKind::Cubic {
                    ha: Vec2 { x: kx, y: 0.0 },    // control from bottom point going right
                    hb: Vec2 { x: 0.0, y: ky },    // control from right point going down
                    mode: HandleMode::Free,
                };
            }
            edge_ids.push(e);
        }

        // Create closed shape from edges
        let shape_id = self.create_shape(&edge_ids, true).unwrap_or(0);

        PrimitiveResult {
            nodes: node_ids,
            edges: edge_ids,
            shape: shape_id,
        }
    }

    // Regions & fills
    pub fn set_flatten_tolerance(&mut self, tol: f32) {
        let tol = tol.max(0.01).min(10.0);
        if (tol - self.flatten_tol).abs() <= f32::EPSILON {
            return;
        }
        self.flatten_tol = tol;
        self.region_cache.borrow_mut().take();
        self.flatten_index.borrow_mut().take();
        self.flatten_cache.borrow_mut().take();
        self.incr_plan.borrow_mut().take();
        self.mark_full_dirty();
        self.bump();
    }
    pub fn get_regions(&mut self) -> Vec<serde_json::Value> {
        algorithms::regions::get_regions_with_fill(self)
    }
    pub fn toggle_region(&mut self, key: u32) -> bool {
        let cur = self.fills.get(&key).copied().unwrap_or(FillState {
            filled: true,
            color: None,
        });
        let next = !cur.filled;
        self.fills.insert(
            key,
            FillState {
                filled: next,
                color: cur.color,
            },
        );
        next
    }
    pub fn set_region_fill(&mut self, key: u32, filled: bool) {
        let color = self.fills.get(&key).and_then(|st| st.color);
        self.fills.insert(key, FillState { filled, color });
    }
    pub fn set_region_color(&mut self, key: u32, r: u8, g: u8, b: u8, a: u8) {
        let filled = self.fills.get(&key).map(|st| st.filled).unwrap_or(true);
        self.fills.insert(
            key,
            FillState {
                filled,
                color: Some(Color { r, g, b, a }),
            },
        );
    }

    #[cfg(feature = "bench_regions")]
    pub fn bench_recompute_regions_full(&mut self) -> usize {
        self.dirty.full = true;
        crate::algorithms::regions::compute_regions_incremental(self).len()
    }

    #[cfg(feature = "bench_regions")]
    pub fn bench_recompute_regions_incremental(&mut self) -> usize {
        crate::algorithms::regions::compute_regions_incremental(self).len()
    }

    // Polyline
    pub fn set_edge_polyline(&mut self, id: u32, points: &[(f32, f32)]) -> bool {
        let changed = {
            let edge = match self.edges.get_mut(id as usize) {
                Some(Some(edge)) => edge,
                _ => return false,
            };
            let new_points: Vec<Vec2> = points.iter().map(|(x, y)| Vec2 { x: *x, y: *y }).collect();
            if let EdgeKind::Polyline { points: ref old } = edge.kind {
                if old.len() == new_points.len()
                    && old.iter().zip(&new_points).all(|(a, b)| {
                        let dx = a.x - b.x;
                        let dy = a.y - b.y;
                        dx * dx + dy * dy
                            <= crate::geometry::tolerance::EPS_POS
                                * crate::geometry::tolerance::EPS_POS
                    })
                {
                    return true;
                }
            }
            edge.kind = EdgeKind::Polyline { points: new_points };
            true
        };
        if changed {
            self.mark_edge_endpoints_dirty(id, 12.0);
            self.bump();
        }
        true
    }
    pub fn get_polyline_points(&self, id: u32) -> Option<Vec<(f32, f32)>> {
        if let Some(Some(edge)) = self.edges.get(id as usize) {
            if let EdgeKind::Polyline { points } = &edge.kind {
                return Some(points.iter().map(|p| (p.x, p.y)).collect());
            }
        }
        None
    }
    pub fn add_polyline_edge(&mut self, a: u32, b: u32, points: &[(f32, f32)]) -> Option<u32> {
        if a == b {
            return None;
        }
        if self
            .nodes
            .get(a as usize)
            .and_then(|n| n.as_ref())
            .is_none()
        {
            return None;
        }
        if self
            .nodes
            .get(b as usize)
            .and_then(|n| n.as_ref())
            .is_none()
        {
            return None;
        }
        let id = self.edges.len() as u32;
        let pts = points.iter().map(|(x, y)| Vec2 { x: *x, y: *y }).collect();
        self.edges.push(Some(Edge {
            a,
            b,
            kind: EdgeKind::Polyline { points: pts },
            stroke: None,
            stroke_width: 2.0,
        }));
        self.dirty.edges_added.insert(id);
        if let (Some(na), Some(nb)) = (
            self.nodes.get(a as usize).and_then(|n| *n),
            self.nodes.get(b as usize).and_then(|n| *n),
        ) {
            let minx = na.x.min(nb.x);
            let maxx = na.x.max(nb.x);
            let miny = na.y.min(nb.y);
            let maxy = na.y.max(nb.y);
            self.expand_dirty_bbox_box(Some((minx, miny, maxx, maxy)));
        }
        self.bump();
        Some(id)
    }

    // SVG
    pub fn add_svg_path(&mut self, d: &str, style: Option<(u8, u8, u8, u8, f32)>) -> u32 {
        svg::add_svg_path_impl(self, d, style)
    }
    pub fn to_svg_paths(&self) -> Vec<String> {
        svg::to_svg_paths_impl(self)
    }

    fn bump(&mut self) {
        self.geom_ver = self.geom_ver.wrapping_add(1);
    }
}

// Layer and group management
impl Graph {
    /// Create a new layer, returns layer ID
    pub fn create_layer(&mut self, name: String) -> LayerId {
        self.layer_system.create_layer(name)
    }

    /// Remove a layer and optionally its edges
    pub fn remove_layer(&mut self, id: LayerId, remove_edges: bool) -> bool {
        if let Some(removed_edges) = self.layer_system.remove_layer(id) {
            if remove_edges {
                for eid in removed_edges {
                    self.remove_edge(eid);
                }
            }
            true
        } else {
            false
        }
    }

    /// Get all layers as (id, name, z_index, visible, opacity)
    pub fn get_layers(&self) -> Vec<(LayerId, String, i32, bool, f32)> {
        self.layer_system
            .layers
            .iter()
            .map(|l| (l.id, l.name.clone(), l.z_index, l.visible, l.opacity))
            .collect()
    }

    /// Rename a layer
    pub fn rename_layer(&mut self, id: LayerId, name: String) -> bool {
        self.layer_system.rename_layer(id, name)
    }

    /// Set layer visibility
    pub fn set_layer_visibility(&mut self, id: LayerId, visible: bool) -> bool {
        if self.layer_system.set_layer_visibility(id, visible) {
            // Visibility change affects regions
            self.mark_full_dirty();
            self.bump();
            true
        } else {
            false
        }
    }

    /// Set layer opacity
    pub fn set_layer_opacity(&mut self, id: LayerId, opacity: f32) -> bool {
        self.layer_system.set_layer_opacity(id, opacity)
    }

    /// Set layer z-index
    pub fn set_layer_z_index(&mut self, id: LayerId, z: i32) -> bool {
        self.layer_system.set_layer_z_index(id, z)
    }

    /// Create a group within a parent group
    pub fn create_group(&mut self, name: String, parent_id: LayerId) -> Option<LayerId> {
        self.layer_system.create_group(name, parent_id)
    }

    /// Remove a group (edges/children move to parent)
    pub fn remove_group(&mut self, id: LayerId) -> bool {
        self.layer_system.remove_group(id)
    }

    /// Get all groups as (id, name, parent, visible, opacity)
    pub fn get_groups(&self) -> Vec<(LayerId, String, Option<LayerId>, bool, f32)> {
        self.layer_system
            .groups
            .values()
            .map(|g| (g.id, g.name.clone(), g.parent, g.visible, g.opacity))
            .collect()
    }

    /// Rename a group
    pub fn rename_group(&mut self, id: LayerId, name: String) -> bool {
        self.layer_system.rename_group(id, name)
    }

    /// Set group visibility
    pub fn set_group_visibility(&mut self, id: LayerId, visible: bool) -> bool {
        if self.layer_system.set_group_visibility(id, visible) {
            // Visibility change affects regions
            self.mark_full_dirty();
            self.bump();
            true
        } else {
            false
        }
    }

    /// Set group opacity
    pub fn set_group_opacity(&mut self, id: LayerId, opacity: f32) -> bool {
        self.layer_system.set_group_opacity(id, opacity)
    }

    /// Add an edge to a specific group
    pub fn add_edge_to_group(&mut self, edge_id: u32, group_id: LayerId) -> bool {
        self.layer_system.add_edge_to_group(edge_id, group_id)
    }

    /// Get the group containing an edge
    pub fn get_edge_group(&self, edge_id: u32) -> Option<LayerId> {
        self.layer_system.get_edge_group(edge_id)
    }

    /// Get the layer containing an edge
    pub fn get_edge_layer(&self, edge_id: u32) -> Option<LayerId> {
        self.layer_system.get_edge_layer(edge_id)
    }

    /// Check if an edge is visible (considering layer/group visibility)
    pub fn is_edge_visible(&self, edge_id: u32) -> bool {
        self.layer_system.is_edge_visible(edge_id)
    }

    /// Get all visible edge IDs
    pub fn get_visible_edges(&self) -> Vec<u32> {
        self.edges
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if e.is_some() && self.layer_system.is_edge_visible(i as u32) {
                    Some(i as u32)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get effective opacity for an edge
    pub fn get_edge_opacity(&self, edge_id: u32) -> f32 {
        self.layer_system.edge_opacity(edge_id)
    }

    /// Get the default group ID (for assigning new edges)
    pub fn default_group(&self) -> Option<LayerId> {
        self.layer_system.default_group()
    }
}

// Gradient management
impl Graph {
    /// Add a linear gradient, returns gradient ID
    pub fn add_linear_gradient(
        &mut self,
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        stops: Vec<ColorStop>,
        units: GradientUnits,
        spread: SpreadMethod,
    ) -> GradientId {
        let id = self.next_gradient_id;
        self.next_gradient_id += 1;
        self.gradients.insert(
            id,
            Gradient::Linear(LinearGradient {
                x1,
                y1,
                x2,
                y2,
                stops,
                units,
                spread,
            }),
        );
        id
    }

    /// Add a radial gradient, returns gradient ID
    pub fn add_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        fx: f32,
        fy: f32,
        stops: Vec<ColorStop>,
        units: GradientUnits,
        spread: SpreadMethod,
    ) -> GradientId {
        let id = self.next_gradient_id;
        self.next_gradient_id += 1;
        self.gradients.insert(
            id,
            Gradient::Radial(RadialGradient {
                cx,
                cy,
                r,
                fx,
                fy,
                stops,
                units,
                spread,
            }),
        );
        id
    }

    /// Update an existing gradient
    pub fn update_gradient(&mut self, id: GradientId, gradient: Gradient) -> bool {
        if self.gradients.contains_key(&id) {
            self.gradients.insert(id, gradient);
            true
        } else {
            false
        }
    }

    /// Remove a gradient
    pub fn remove_gradient(&mut self, id: GradientId) -> bool {
        self.gradients.remove(&id).is_some()
    }

    /// Get a gradient by ID
    pub fn get_gradient(&self, id: GradientId) -> Option<&Gradient> {
        self.gradients.get(&id)
    }

    /// Get all gradient IDs
    pub fn gradient_ids(&self) -> Vec<GradientId> {
        self.gradients.keys().copied().collect()
    }

    /// Get all gradients as (id, gradient) pairs
    pub fn get_all_gradients(&self) -> Vec<(GradientId, &Gradient)> {
        self.gradients.iter().map(|(id, g)| (*id, g)).collect()
    }

    /// Set a region fill to use a gradient
    pub fn set_region_gradient(&mut self, key: u32, gradient_id: GradientId) -> bool {
        if !self.gradients.contains_key(&gradient_id) {
            return false;
        }
        let filled = self.fills.get(&key).map(|st| st.filled).unwrap_or(true);
        // For now, we store the gradient reference in color field as a marker
        // Full implementation would require updating FillState to use Paint
        self.fills.insert(
            key,
            FillState {
                filled,
                color: None, // Gradient reference stored separately
            },
        );
        true
    }

    /// Set edge stroke to use a gradient
    pub fn set_edge_stroke_gradient(
        &mut self,
        id: u32,
        gradient_id: GradientId,
        width: f32,
    ) -> bool {
        if !self.gradients.contains_key(&gradient_id) {
            return false;
        }
        if let Some(Some(e)) = self.edges.get_mut(id as usize) {
            // For now, we clear the stroke color to indicate gradient use
            // Full implementation would require updating Edge to use Paint
            e.stroke = None;
            e.stroke_width = if width > 0.0 { width } else { 2.0 };
            return true;
        }
        false
    }
}

// Transforms and grouping moves
impl Graph {
    pub fn transform_all(&mut self, s: f32, tx: f32, ty: f32, scale_stroke: bool) {
        for n in self.nodes.iter_mut() {
            if let Some(n) = n {
                n.x = n.x * s + tx;
                n.y = n.y * s + ty;
            }
        }
        for e in self.edges.iter_mut() {
            if let Some(e) = e {
                match &mut e.kind {
                    EdgeKind::Line => {}
                    EdgeKind::Cubic { ha, hb, .. } => {
                        ha.x *= s;
                        ha.y *= s;
                        hb.x *= s;
                        hb.y *= s;
                    }
                    EdgeKind::Polyline { points } => {
                        for p in points {
                            p.x = p.x * s + tx;
                            p.y = p.y * s + ty;
                        }
                    }
                }
                if scale_stroke {
                    e.stroke_width *= s;
                }
            }
        }
        self.region_cache.borrow_mut().take();
        self.flatten_index.borrow_mut().take();
        self.flatten_cache.borrow_mut().take();
        self.incr_plan.borrow_mut().take();
        self.mark_full_dirty();
        self.bump();
    }
    pub fn translate_nodes(&mut self, ids: &[u32], dx: f32, dy: f32) -> u32 {
        let mut moved = 0;
        for &id in ids {
            if let Some((x, y)) = self.get_node(id) {
                if self.move_node(id, x + dx, y + dy) {
                    moved += 1;
                }
            }
        }
        moved
    }
    pub fn translate_edges(
        &mut self,
        edge_ids: &[u32],
        dx: f32,
        dy: f32,
        split_shared: bool,
    ) -> u32 {
        let mut nodes_to_move: HashSet<u32> = HashSet::new();
        for &eid in edge_ids {
            if let Some(e) = self.edges.get(eid as usize).and_then(|e| e.as_ref()) {
                nodes_to_move.insert(e.a);
                nodes_to_move.insert(e.b);
            }
        }
        if split_shared {
            let selected: HashSet<u32> = edge_ids.iter().copied().collect();
            let mut remap: HashMap<u32, u32> = HashMap::new();
            for nid in nodes_to_move.clone().into_iter() {
                let mut used_elsewhere = false;
                for (i, e) in self.edges.iter().enumerate() {
                    if let Some(e) = e {
                        if (e.a == nid || e.b == nid) && !selected.contains(&(i as u32)) {
                            used_elsewhere = true;
                            break;
                        }
                    }
                }
                if used_elsewhere {
                    if let Some((x, y)) = self.get_node(nid) {
                        let new_id = self.add_node(x, y);
                        remap.insert(nid, new_id);
                    }
                }
            }
            if !remap.is_empty() {
                for &eid in edge_ids {
                    if let Some(Some(e)) = self.edges.get_mut(eid as usize) {
                        if let Some(&na) = remap.get(&e.a) {
                            e.a = na;
                            self.dirty.edges_modified.insert(eid as u32);
                        }
                        if let Some(&nb) = remap.get(&e.b) {
                            e.b = nb;
                            self.dirty.edges_modified.insert(eid as u32);
                        }
                    }
                }
                nodes_to_move = remap.values().copied().collect();
            }
        }
        let mut moved = 0;
        for nid in nodes_to_move {
            if let Some((x, y)) = self.get_node(nid) {
                if self.move_node(nid, x + dx, y + dy) {
                    moved += 1;
                }
            }
        }
        moved
    }
}

// Shape management
impl Graph {
    /// Create a shape from an ordered list of edge IDs.
    ///
    /// Returns the shape ID, or None if any edge ID is invalid.
    pub fn create_shape(&mut self, edge_ids: &[u32], closed: bool) -> Option<u32> {
        // Validate all edge IDs exist
        for &eid in edge_ids {
            if self.edges.get(eid as usize).and_then(|e| e.as_ref()).is_none() {
                return None;
            }
        }

        let id = self.shapes.len() as u32;
        self.shapes.push(Some(Shape {
            id,
            edges: edge_ids.to_vec(),
            closed,
            fill_rule: FillRule::NonZero,
        }));
        Some(id)
    }

    /// Create a shape with a specific fill rule.
    pub fn create_shape_with_fill_rule(
        &mut self,
        edge_ids: &[u32],
        closed: bool,
        fill_rule: FillRule,
    ) -> Option<u32> {
        // Validate all edge IDs exist
        for &eid in edge_ids {
            if self.edges.get(eid as usize).and_then(|e| e.as_ref()).is_none() {
                return None;
            }
        }

        let id = self.shapes.len() as u32;
        self.shapes.push(Some(Shape {
            id,
            edges: edge_ids.to_vec(),
            closed,
            fill_rule,
        }));
        Some(id)
    }

    /// Delete a shape by ID. Returns true if the shape existed.
    pub fn delete_shape(&mut self, id: u32) -> bool {
        if let Some(slot) = self.shapes.get_mut(id as usize) {
            if slot.is_some() {
                *slot = None;
                return true;
            }
        }
        false
    }

    /// Get a reference to a shape by ID.
    pub fn get_shape(&self, id: u32) -> Option<&Shape> {
        self.shapes.get(id as usize).and_then(|s| s.as_ref())
    }

    /// Get the edge IDs that form a shape.
    pub fn get_shape_edges(&self, id: u32) -> Option<&[u32]> {
        self.get_shape(id).map(|s| s.edges.as_slice())
    }

    /// Get all shape IDs.
    pub fn get_shape_ids(&self) -> Vec<u32> {
        self.shapes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|_| i as u32))
            .collect()
    }

    /// Count the number of shapes.
    pub fn shape_count(&self) -> u32 {
        self.shapes.iter().filter(|s| s.is_some()).count() as u32
    }

    /// Set the fill rule for a shape.
    pub fn set_shape_fill_rule(&mut self, id: u32, fill_rule: FillRule) -> bool {
        if let Some(Some(shape)) = self.shapes.get_mut(id as usize) {
            shape.fill_rule = fill_rule;
            return true;
        }
        false
    }

    /// Infer shapes from closed loops in the graph.
    ///
    /// This finds cycles of connected edges and creates shapes for each.
    /// Returns the IDs of newly created shapes.
    pub fn infer_shapes(&mut self) -> Vec<u32> {
        // Build adjacency: node -> list of (edge_id, other_node)
        let mut adj: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
        for (eid, edge_opt) in self.edges.iter().enumerate() {
            if let Some(edge) = edge_opt {
                adj.entry(edge.a).or_default().push((eid as u32, edge.b));
                adj.entry(edge.b).or_default().push((eid as u32, edge.a));
            }
        }

        // Collect edges already in shapes
        let mut used_edges: HashSet<u32> = HashSet::new();
        for shape_opt in &self.shapes {
            if let Some(shape) = shape_opt {
                for &eid in &shape.edges {
                    used_edges.insert(eid);
                }
            }
        }

        // First pass: find all cycles (without mutating self)
        let mut found_paths: Vec<Vec<u32>> = Vec::new();
        let mut visited_edges: HashSet<u32> = HashSet::new();

        for (start_eid, edge_opt) in self.edges.iter().enumerate() {
            let start_eid = start_eid as u32;
            if edge_opt.is_none() || used_edges.contains(&start_eid) || visited_edges.contains(&start_eid) {
                continue;
            }

            let edge = edge_opt.as_ref().unwrap();
            let start_node = edge.a;

            // Try to find a path back to start_node
            if let Some(path) = self.find_cycle_from(start_eid, start_node, &adj, &used_edges) {
                // Mark all edges in path as visited
                for &eid in &path {
                    visited_edges.insert(eid);
                }
                found_paths.push(path);
            }
        }

        // Second pass: create shapes from found paths
        let mut created_shapes = Vec::new();
        for path in found_paths {
            if let Some(shape_id) = self.create_shape(&path, true) {
                created_shapes.push(shape_id);
            }
        }

        created_shapes
    }

    /// Helper to find a cycle starting from an edge and returning to start_node.
    /// Uses DFS with backtracking to handle complex graph topologies.
    fn find_cycle_from(
        &self,
        start_edge: u32,
        start_node: u32,
        adj: &HashMap<u32, Vec<(u32, u32)>>,
        used_edges: &HashSet<u32>,
    ) -> Option<Vec<u32>> {
        let edge = self.edges.get(start_edge as usize)?.as_ref()?;
        let first_node = if edge.a == start_node { edge.b } else { edge.a };

        // DFS with explicit stack: (current_node, path, visited_nodes, neighbor_index)
        let mut stack: Vec<(u32, Vec<u32>, HashSet<u32>, usize)> = Vec::new();

        let mut initial_visited = HashSet::new();
        initial_visited.insert(start_node);
        stack.push((first_node, vec![start_edge], initial_visited, 0));

        let max_iterations = self.edges.len() * self.edges.len();
        let mut iterations = 0;

        while let Some((current_node, path, mut visited, neighbor_idx)) = stack.pop() {
            iterations += 1;
            if iterations > max_iterations {
                break;
            }

            // Check if we completed the cycle
            if current_node == start_node && path.len() > 1 {
                return Some(path);
            }

            // Skip if we've visited this node already in this path
            if visited.contains(&current_node) {
                continue;
            }

            let neighbors = match adj.get(&current_node) {
                Some(n) => n,
                None => continue,
            };

            // Find next unvisited edges from this node
            for i in neighbor_idx..neighbors.len() {
                let (eid, other) = neighbors[i];
                if !path.contains(&eid) && !used_edges.contains(&eid) {
                    // Push current state back with incremented index for backtracking
                    stack.push((current_node, path.clone(), visited.clone(), i + 1));

                    // Push new state
                    let mut new_path = path.clone();
                    new_path.push(eid);
                    let mut new_visited = visited.clone();
                    new_visited.insert(current_node);
                    stack.push((other, new_path, new_visited, 0));
                    break;
                }
            }

            // If no more neighbors to try, this branch is exhausted (backtrack)
        }

        None
    }
}

// Text management
impl Graph {
    /// Add a simple text label at the specified position.
    /// Returns the text ID.
    pub fn add_text(&mut self, content: &str, x: f32, y: f32) -> TextId {
        let id = self.texts.len() as TextId;
        self.texts.push(Some(TextElement::new_label(id, content.to_string(), x, y)));
        id
    }

    /// Add a text box with wrapping at the specified position.
    /// Returns the text ID.
    pub fn add_text_box(&mut self, content: &str, x: f32, y: f32, width: f32, height: f32) -> TextId {
        let id = self.texts.len() as TextId;
        self.texts.push(Some(TextElement::new_box(id, content.to_string(), x, y, width, height)));
        id
    }

    /// Add text on a path defined by edge IDs.
    /// Returns the text ID.
    pub fn add_text_on_path(&mut self, content: &str, edge_ids: Vec<u32>) -> TextId {
        let id = self.texts.len() as TextId;
        self.texts.push(Some(TextElement::new_on_path(id, content.to_string(), edge_ids)));
        id
    }

    /// Remove a text element by ID. Returns true if it existed.
    pub fn remove_text(&mut self, id: TextId) -> bool {
        if let Some(slot) = self.texts.get_mut(id as usize) {
            if slot.is_some() {
                *slot = None;
                return true;
            }
        }
        false
    }

    /// Get a reference to a text element by ID.
    pub fn get_text(&self, id: TextId) -> Option<&TextElement> {
        self.texts.get(id as usize).and_then(|t| t.as_ref())
    }

    /// Get a mutable reference to a text element by ID.
    pub fn get_text_mut(&mut self, id: TextId) -> Option<&mut TextElement> {
        self.texts.get_mut(id as usize).and_then(|t| t.as_mut())
    }

    /// Get all text IDs.
    pub fn get_text_ids(&self) -> Vec<TextId> {
        self.texts
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.as_ref().map(|_| i as TextId))
            .collect()
    }

    /// Count the number of text elements.
    pub fn text_count(&self) -> u32 {
        self.texts.iter().filter(|t| t.is_some()).count() as u32
    }

    /// Set the content of a text element.
    pub fn set_text_content(&mut self, id: TextId, content: &str) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.content = content.to_string();
            return true;
        }
        false
    }

    /// Set the position of a text element.
    pub fn set_text_position(&mut self, id: TextId, x: f32, y: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.position = Vec2 { x, y };
            return true;
        }
        false
    }

    /// Set the rotation of a text element (in radians).
    pub fn set_text_rotation(&mut self, id: TextId, radians: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.rotation = radians;
            return true;
        }
        false
    }

    /// Set the text alignment.
    pub fn set_text_align(&mut self, id: TextId, align: TextAlign) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.align = align;
            return true;
        }
        false
    }

    /// Set the complete style of a text element.
    pub fn set_text_style(&mut self, id: TextId, style: TextStyle) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style = style;
            return true;
        }
        false
    }

    /// Set individual font properties.
    pub fn set_text_font(&mut self, id: TextId, font_family: &str, font_size: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.font_family = font_family.to_string();
            text.style.font_size = font_size;
            return true;
        }
        false
    }

    /// Set font weight (100-900).
    pub fn set_text_font_weight(&mut self, id: TextId, weight: u16) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.font_weight = weight.clamp(100, 900);
            return true;
        }
        false
    }

    /// Set font style (normal, italic, oblique).
    pub fn set_text_font_style(&mut self, id: TextId, style: FontStyle) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.font_style = style;
            return true;
        }
        false
    }

    /// Set the fill color of text.
    pub fn set_text_fill_color(&mut self, id: TextId, r: u8, g: u8, b: u8, a: u8) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.fill_color = Some(Color { r, g, b, a });
            return true;
        }
        false
    }

    /// Clear the fill color (make text transparent fill).
    pub fn clear_text_fill_color(&mut self, id: TextId) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.fill_color = None;
            return true;
        }
        false
    }

    /// Set the stroke color of text.
    pub fn set_text_stroke_color(&mut self, id: TextId, r: u8, g: u8, b: u8, a: u8) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.stroke_color = Some(Color { r, g, b, a });
            return true;
        }
        false
    }

    /// Set the stroke width of text.
    pub fn set_text_stroke_width(&mut self, id: TextId, width: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.stroke_width = width.max(0.0);
            return true;
        }
        false
    }

    /// Set letter spacing (in em units).
    pub fn set_text_letter_spacing(&mut self, id: TextId, spacing: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.letter_spacing = spacing;
            return true;
        }
        false
    }

    /// Set line height multiplier.
    pub fn set_text_line_height(&mut self, id: TextId, line_height: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.style.line_height = line_height.max(0.1);
            return true;
        }
        false
    }

    /// Convert a text label to a text box.
    pub fn convert_text_to_box(&mut self, id: TextId, width: f32, height: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.text_type = TextType::Box {
                width,
                height,
                vertical_align: VerticalAlign::Top,
                overflow: TextOverflow::Clip,
            };
            return true;
        }
        false
    }

    /// Convert a text element to text on path.
    pub fn convert_text_to_on_path(&mut self, id: TextId, edge_ids: Vec<u32>, start_offset: f32) -> bool {
        // Validate edge IDs
        for &eid in &edge_ids {
            if self.edges.get(eid as usize).and_then(|e| e.as_ref()).is_none() {
                return false;
            }
        }
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.text_type = TextType::OnPath {
                edge_ids,
                start_offset: start_offset.clamp(0.0, 1.0),
            };
            return true;
        }
        false
    }

    /// Convert a text element back to a simple label.
    pub fn convert_text_to_label(&mut self, id: TextId) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.text_type = TextType::Label;
            return true;
        }
        false
    }

    /// Set text box dimensions (only for text box type).
    pub fn set_text_box_size(&mut self, id: TextId, width: f32, height: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            if let TextType::Box { vertical_align, overflow, .. } = text.text_type {
                text.text_type = TextType::Box {
                    width,
                    height,
                    vertical_align,
                    overflow,
                };
                return true;
            }
        }
        false
    }

    /// Set text box vertical alignment.
    pub fn set_text_box_vertical_align(&mut self, id: TextId, align: VerticalAlign) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            if let TextType::Box { width, height, overflow, .. } = text.text_type {
                text.text_type = TextType::Box {
                    width,
                    height,
                    vertical_align: align,
                    overflow,
                };
                return true;
            }
        }
        false
    }

    /// Set text box overflow behavior.
    pub fn set_text_box_overflow(&mut self, id: TextId, overflow: TextOverflow) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            if let TextType::Box { width, height, vertical_align, .. } = text.text_type {
                text.text_type = TextType::Box {
                    width,
                    height,
                    vertical_align,
                    overflow,
                };
                return true;
            }
        }
        false
    }

    /// Set the start offset for text on path (0.0 to 1.0).
    pub fn set_text_path_offset(&mut self, id: TextId, offset: f32) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            if let TextType::OnPath { edge_ids, .. } = &text.text_type {
                let edge_ids = edge_ids.clone();
                text.text_type = TextType::OnPath {
                    edge_ids,
                    start_offset: offset.clamp(0.0, 1.0),
                };
                return true;
            }
        }
        false
    }

    /// Update the path for text on path.
    pub fn set_text_path_edges(&mut self, id: TextId, edge_ids: Vec<u32>) -> bool {
        // Validate edge IDs
        for &eid in &edge_ids {
            if self.edges.get(eid as usize).and_then(|e| e.as_ref()).is_none() {
                return false;
            }
        }
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            if let TextType::OnPath { start_offset, .. } = text.text_type {
                text.text_type = TextType::OnPath {
                    edge_ids,
                    start_offset,
                };
                return true;
            }
        }
        false
    }
}
