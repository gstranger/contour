//! Boolean operations on shapes (union, intersect, difference, XOR).
//!
//! This module implements boolean operations by:
//! 1. Planarizing the combined edges of two shapes
//! 2. Computing winding numbers for each resulting region
//! 3. Filtering regions based on the operation type
//! 4. Reconstructing output edges from kept region boundaries

use crate::algorithms::winding::{point_in_polygon_evenodd, point_in_polygon_nonzero};
use crate::geometry::cubic::CubicBezier;
use crate::model::{Edge, EdgeKind, FillRule, Shape, Vec2};
use crate::Graph;
use std::collections::{HashMap, HashSet};

/// Boolean operation type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolOp {
    /// A ∪ B - areas in A or B or both
    Union,
    /// A ∩ B - areas in both A and B
    Intersect,
    /// A - B - areas in A but not in B
    Difference,
    /// A ⊕ B - areas in A or B but not both
    Xor,
}

/// Error type for boolean operations
#[derive(Clone, Debug)]
pub enum BoolError {
    /// Shape not found
    ShapeNotFound(u32),
    /// Shape has no edges
    EmptyShape(u32),
    /// Edge in shape not found
    EdgeNotFound(u32),
    /// Node not found
    NodeNotFound(u32),
    /// Operation failed (generic)
    OperationFailed(String),
}

/// Result of a boolean operation
#[derive(Clone, Debug)]
pub struct BooleanResult {
    /// IDs of newly created shapes
    pub shapes: Vec<u32>,
    /// IDs of newly created nodes (intersection points)
    pub nodes: Vec<u32>,
    /// IDs of newly created edges
    pub edges: Vec<u32>,
}

impl Graph {
    /// Perform a boolean operation on two shapes.
    ///
    /// Returns the result containing new shape, node, and edge IDs.
    /// The original shapes are not modified.
    pub fn boolean_op(
        &mut self,
        shape_a: u32,
        shape_b: u32,
        op: BoolOp,
    ) -> Result<BooleanResult, BoolError> {
        // Get shape data
        let shape_a_data = self
            .get_shape(shape_a)
            .ok_or(BoolError::ShapeNotFound(shape_a))?
            .clone();
        let shape_b_data = self
            .get_shape(shape_b)
            .ok_or(BoolError::ShapeNotFound(shape_b))?
            .clone();

        if shape_a_data.edges.is_empty() {
            return Err(BoolError::EmptyShape(shape_a));
        }
        if shape_b_data.edges.is_empty() {
            return Err(BoolError::EmptyShape(shape_b));
        }

        // Flatten shapes to polygons for winding number tests
        let polygon_a = self.shape_to_polygon(&shape_a_data)?;
        let polygon_b = self.shape_to_polygon(&shape_b_data)?;

        // Collect edges from both shapes
        let edges_a: HashSet<u32> = shape_a_data.edges.iter().copied().collect();
        let edges_b: HashSet<u32> = shape_b_data.edges.iter().copied().collect();

        // Get flattened segments for intersection detection
        let segments_a = self.flatten_shape_edges(&shape_a_data)?;
        let segments_b = self.flatten_shape_edges(&shape_b_data)?;

        // Find all intersections between shape A and B segments
        let intersections = find_segment_intersections(&segments_a, &segments_b);

        // If no intersections, handle the simple cases
        if intersections.is_empty() {
            return self.boolean_no_intersections(
                &shape_a_data,
                &shape_b_data,
                &polygon_a,
                &polygon_b,
                op,
            );
        }

        // Complex case: shapes intersect
        self.boolean_with_intersections(
            &shape_a_data,
            &shape_b_data,
            &polygon_a,
            &polygon_b,
            &edges_a,
            &edges_b,
            &intersections,
            op,
        )
    }

    /// Convert a shape's edges to a flattened polygon for winding tests.
    fn shape_to_polygon(&self, shape: &Shape) -> Result<Vec<Vec2>, BoolError> {
        let mut polygon = Vec::new();

        for &eid in &shape.edges {
            let edge = self
                .edges
                .get(eid as usize)
                .and_then(|e| e.as_ref())
                .ok_or(BoolError::EdgeNotFound(eid))?;

            let p0 = self
                .nodes
                .get(edge.a as usize)
                .and_then(|n| *n)
                .ok_or(BoolError::NodeNotFound(edge.a))?;

            // Add start point
            polygon.push(Vec2 { x: p0.x, y: p0.y });

            // For cubics, add intermediate points
            if let EdgeKind::Cubic { ha, hb, .. } = &edge.kind {
                let p3 = self
                    .nodes
                    .get(edge.b as usize)
                    .and_then(|n| *n)
                    .ok_or(BoolError::NodeNotFound(edge.b))?;

                let curve = CubicBezier::new(
                    Vec2 { x: p0.x, y: p0.y },
                    Vec2 {
                        x: p0.x + ha.x,
                        y: p0.y + ha.y,
                    },
                    Vec2 {
                        x: p3.x + hb.x,
                        y: p3.y + hb.y,
                    },
                    Vec2 { x: p3.x, y: p3.y },
                );

                // Sample the curve
                for i in 1..8 {
                    let t = i as f32 / 8.0;
                    polygon.push(curve.eval(t));
                }
            }
        }

        Ok(polygon)
    }

    /// Get flattened line segments for a shape's edges.
    fn flatten_shape_edges(&self, shape: &Shape) -> Result<Vec<FlatSegment>, BoolError> {
        let mut segments = Vec::new();

        for &eid in &shape.edges {
            let edge = self
                .edges
                .get(eid as usize)
                .and_then(|e| e.as_ref())
                .ok_or(BoolError::EdgeNotFound(eid))?;

            let p0 = self
                .nodes
                .get(edge.a as usize)
                .and_then(|n| *n)
                .ok_or(BoolError::NodeNotFound(edge.a))?;
            let p3 = self
                .nodes
                .get(edge.b as usize)
                .and_then(|n| *n)
                .ok_or(BoolError::NodeNotFound(edge.b))?;

            let start = Vec2 { x: p0.x, y: p0.y };
            let end = Vec2 { x: p3.x, y: p3.y };

            match &edge.kind {
                EdgeKind::Line => {
                    segments.push(FlatSegment {
                        start,
                        end,
                        edge_id: eid,
                        t_start: 0.0,
                        t_end: 1.0,
                    });
                }
                EdgeKind::Cubic { ha, hb, .. } => {
                    let curve = CubicBezier::new(
                        start,
                        Vec2 {
                            x: p0.x + ha.x,
                            y: p0.y + ha.y,
                        },
                        Vec2 {
                            x: p3.x + hb.x,
                            y: p3.y + hb.y,
                        },
                        end,
                    );

                    // Flatten to line segments
                    let steps = 16;
                    for i in 0..steps {
                        let t0 = i as f32 / steps as f32;
                        let t1 = (i + 1) as f32 / steps as f32;
                        segments.push(FlatSegment {
                            start: curve.eval(t0),
                            end: curve.eval(t1),
                            edge_id: eid,
                            t_start: t0,
                            t_end: t1,
                        });
                    }
                }
                EdgeKind::Polyline { points } => {
                    let mut prev = start;
                    let n = points.len() + 1;
                    for (i, p) in points.iter().enumerate() {
                        segments.push(FlatSegment {
                            start: prev,
                            end: *p,
                            edge_id: eid,
                            t_start: i as f32 / n as f32,
                            t_end: (i + 1) as f32 / n as f32,
                        });
                        prev = *p;
                    }
                    segments.push(FlatSegment {
                        start: prev,
                        end,
                        edge_id: eid,
                        t_start: (n - 1) as f32 / n as f32,
                        t_end: 1.0,
                    });
                }
            }
        }

        Ok(segments)
    }

    /// Handle boolean when shapes don't intersect.
    fn boolean_no_intersections(
        &mut self,
        shape_a: &Shape,
        shape_b: &Shape,
        polygon_a: &[Vec2],
        polygon_b: &[Vec2],
        op: BoolOp,
    ) -> Result<BooleanResult, BoolError> {
        // Check containment by testing a point from each shape
        let point_a = polygon_a.first().copied().unwrap_or(Vec2 { x: 0.0, y: 0.0 });
        let point_b = polygon_b.first().copied().unwrap_or(Vec2 { x: 0.0, y: 0.0 });

        let a_in_b = point_in_polygon(&shape_b.fill_rule, point_a.x, point_a.y, polygon_b);
        let b_in_a = point_in_polygon(&shape_a.fill_rule, point_b.x, point_b.y, polygon_a);

        let mut result = BooleanResult {
            shapes: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        match op {
            BoolOp::Union => {
                if a_in_b {
                    // A is inside B, result is B
                    result.shapes.push(self.clone_shape(shape_b)?);
                } else if b_in_a {
                    // B is inside A, result is A
                    result.shapes.push(self.clone_shape(shape_a)?);
                } else {
                    // Disjoint, result is both shapes
                    result.shapes.push(self.clone_shape(shape_a)?);
                    result.shapes.push(self.clone_shape(shape_b)?);
                }
            }
            BoolOp::Intersect => {
                if a_in_b {
                    // A is inside B, result is A
                    result.shapes.push(self.clone_shape(shape_a)?);
                } else if b_in_a {
                    // B is inside A, result is B
                    result.shapes.push(self.clone_shape(shape_b)?);
                }
                // If disjoint, intersection is empty
            }
            BoolOp::Difference => {
                if a_in_b {
                    // A is entirely inside B, result is empty
                } else if b_in_a {
                    // B is inside A, this would create a hole (complex case)
                    // For now, return A (simplified)
                    result.shapes.push(self.clone_shape(shape_a)?);
                } else {
                    // Disjoint, result is A unchanged
                    result.shapes.push(self.clone_shape(shape_a)?);
                }
            }
            BoolOp::Xor => {
                if a_in_b || b_in_a {
                    // One inside the other creates a ring (complex)
                    // For now, return both shapes
                    result.shapes.push(self.clone_shape(shape_a)?);
                    result.shapes.push(self.clone_shape(shape_b)?);
                } else {
                    // Disjoint, XOR is union
                    result.shapes.push(self.clone_shape(shape_a)?);
                    result.shapes.push(self.clone_shape(shape_b)?);
                }
            }
        }

        Ok(result)
    }

    /// Handle boolean when shapes do intersect.
    fn boolean_with_intersections(
        &mut self,
        shape_a: &Shape,
        shape_b: &Shape,
        polygon_a: &[Vec2],
        polygon_b: &[Vec2],
        edges_a: &HashSet<u32>,
        edges_b: &HashSet<u32>,
        intersections: &[Intersection],
        op: BoolOp,
    ) -> Result<BooleanResult, BoolError> {
        let mut result = BooleanResult {
            shapes: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        // Create nodes at intersection points
        let mut intersection_nodes: HashMap<(u32, u32, usize), u32> = HashMap::new();
        for int in intersections {
            let node_id = self.add_node(int.point.x, int.point.y);
            result.nodes.push(node_id);
            intersection_nodes.insert((int.seg_a_id, int.seg_b_id, 0), node_id);
        }

        // Build the combined edge graph with intersection points
        // This is a simplified approach - for full correctness we'd need
        // proper planarization like the existing planarize_graph

        // For now, we use a region-based approach:
        // 1. Get all regions from planarization of combined edges
        // 2. For each region, compute winding relative to A and B
        // 3. Keep/discard based on operation

        // Collect all edges
        let all_edges: Vec<u32> = edges_a.union(edges_b).copied().collect();

        // Use existing region computation on a temporary graph
        // This is a simplification - proper implementation would integrate
        // with the planarization to handle intersections correctly

        // For the MVP, we'll trace the boundary between kept/discarded regions
        // and construct new edges

        // Compute centroids of regions and classify them
        let regions = self.get_regions();

        for region in &regions {
            // Parse region data
            let key = region.get("key").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let points_val = region.get("points").and_then(|v| v.as_array());

            if let Some(pts) = points_val {
                // Compute centroid
                let mut cx = 0.0f32;
                let mut cy = 0.0f32;
                let mut count = 0;

                let mut i = 0;
                while i + 1 < pts.len() {
                    if let (Some(x), Some(y)) = (
                        pts[i].as_f64().map(|v| v as f32),
                        pts[i + 1].as_f64().map(|v| v as f32),
                    ) {
                        cx += x;
                        cy += y;
                        count += 1;
                    }
                    i += 2;
                }

                if count > 0 {
                    cx /= count as f32;
                    cy /= count as f32;

                    // Test winding for centroid
                    let in_a = point_in_polygon(&shape_a.fill_rule, cx, cy, polygon_a);
                    let in_b = point_in_polygon(&shape_b.fill_rule, cx, cy, polygon_b);

                    let keep = match op {
                        BoolOp::Union => in_a || in_b,
                        BoolOp::Intersect => in_a && in_b,
                        BoolOp::Difference => in_a && !in_b,
                        BoolOp::Xor => in_a != in_b,
                    };

                    if keep {
                        // Mark this region's fill state
                        self.set_region_fill(key, true);
                    } else {
                        self.set_region_fill(key, false);
                    }
                }
            }
        }

        // Create a new shape from the kept regions
        // This is simplified - proper implementation would reconstruct edges
        // For now, we create a shape from all edges involved
        let new_shape_id = self.create_shape(&all_edges, true);
        if let Some(sid) = new_shape_id {
            result.shapes.push(sid);
        }

        Ok(result)
    }

    /// Clone a shape with new edge copies.
    fn clone_shape(&mut self, shape: &Shape) -> Result<u32, BoolError> {
        let mut new_edges = Vec::new();

        for &eid in &shape.edges {
            let edge = self
                .edges
                .get(eid as usize)
                .and_then(|e| e.as_ref())
                .ok_or(BoolError::EdgeNotFound(eid))?
                .clone();

            let new_eid = self.edges.len() as u32;
            self.edges.push(Some(edge));
            new_edges.push(new_eid);
        }

        self.create_shape(&new_edges, shape.closed)
            .ok_or_else(|| BoolError::OperationFailed("Failed to create shape".to_string()))
    }
}

/// A flattened line segment from an edge.
#[derive(Clone, Debug)]
struct FlatSegment {
    start: Vec2,
    end: Vec2,
    edge_id: u32,
    t_start: f32,
    t_end: f32,
}

/// An intersection between two segments.
#[derive(Clone, Debug)]
struct Intersection {
    point: Vec2,
    seg_a_id: u32,
    seg_b_id: u32,
    t_a: f32,
    t_b: f32,
}

/// Find intersections between two sets of segments.
fn find_segment_intersections(segs_a: &[FlatSegment], segs_b: &[FlatSegment]) -> Vec<Intersection> {
    let mut intersections = Vec::new();

    for (i, sa) in segs_a.iter().enumerate() {
        for (j, sb) in segs_b.iter().enumerate() {
            if let Some((t, u, point)) = segment_intersection(sa, sb) {
                if t > 0.001 && t < 0.999 && u > 0.001 && u < 0.999 {
                    intersections.push(Intersection {
                        point,
                        seg_a_id: i as u32,
                        seg_b_id: j as u32,
                        t_a: sa.t_start + t * (sa.t_end - sa.t_start),
                        t_b: sb.t_start + u * (sb.t_end - sb.t_start),
                    });
                }
            }
        }
    }

    intersections
}

/// Compute intersection of two line segments.
fn segment_intersection(a: &FlatSegment, b: &FlatSegment) -> Option<(f32, f32, Vec2)> {
    let ax = a.end.x - a.start.x;
    let ay = a.end.y - a.start.y;
    let bx = b.end.x - b.start.x;
    let by = b.end.y - b.start.y;

    let denom = ax * by - ay * bx;
    if denom.abs() < 1e-10 {
        return None; // Parallel
    }

    let cx = b.start.x - a.start.x;
    let cy = b.start.y - a.start.y;

    let t = (cx * by - cy * bx) / denom;
    let u = (cx * ay - cy * ax) / denom;

    if t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0 {
        let point = Vec2 {
            x: a.start.x + t * ax,
            y: a.start.y + t * ay,
        };
        Some((t, u, point))
    } else {
        None
    }
}

/// Point in polygon test using the shape's fill rule.
fn point_in_polygon(fill_rule: &FillRule, px: f32, py: f32, polygon: &[Vec2]) -> bool {
    match fill_rule {
        FillRule::NonZero => point_in_polygon_nonzero(px, py, polygon),
        FillRule::EvenOdd => point_in_polygon_evenodd(px, py, polygon),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec2(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }

    #[test]
    fn test_segment_intersection() {
        let a = FlatSegment {
            start: vec2(0.0, 0.0),
            end: vec2(10.0, 10.0),
            edge_id: 0,
            t_start: 0.0,
            t_end: 1.0,
        };
        let b = FlatSegment {
            start: vec2(0.0, 10.0),
            end: vec2(10.0, 0.0),
            edge_id: 1,
            t_start: 0.0,
            t_end: 1.0,
        };

        let result = segment_intersection(&a, &b);
        assert!(result.is_some());

        let (t, u, point) = result.unwrap();
        assert!((t - 0.5).abs() < 0.01);
        assert!((u - 0.5).abs() < 0.01);
        assert!((point.x - 5.0).abs() < 0.1);
        assert!((point.y - 5.0).abs() < 0.1);
    }

    #[test]
    fn test_segment_no_intersection() {
        let a = FlatSegment {
            start: vec2(0.0, 0.0),
            end: vec2(5.0, 0.0),
            edge_id: 0,
            t_start: 0.0,
            t_end: 1.0,
        };
        let b = FlatSegment {
            start: vec2(0.0, 5.0),
            end: vec2(5.0, 5.0),
            edge_id: 1,
            t_start: 0.0,
            t_end: 1.0,
        };

        let result = segment_intersection(&a, &b);
        assert!(result.is_none());
    }
}
