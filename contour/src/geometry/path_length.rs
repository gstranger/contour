//! Path length calculation and point sampling for text-on-path.
//!
//! Provides functions to calculate the length of a path defined by edges,
//! and to sample points along the path at specific distances.

use crate::geometry::cubic::CubicBezier;
use crate::model::{EdgeKind, Vec2};
use crate::Graph;

/// A sampled point on a path with position and tangent angle
#[derive(Debug, Clone, Copy)]
pub struct PathPoint {
    /// X coordinate
    pub x: f32,
    /// Y coordinate
    pub y: f32,
    /// Tangent angle in radians
    pub angle: f32,
}

impl Graph {
    /// Calculate the total length of a path defined by edge IDs.
    pub fn path_length(&self, edge_ids: &[u32]) -> f32 {
        let mut total = 0.0;
        for &eid in edge_ids {
            total += self.edge_length(eid).unwrap_or(0.0);
        }
        total
    }

    /// Calculate the length of a single edge.
    pub fn edge_length(&self, edge_id: u32) -> Option<f32> {
        let edge = self.edges.get(edge_id as usize)?.as_ref()?;
        let (ax, ay) = self.get_node(edge.a)?;
        let (bx, by) = self.get_node(edge.b)?;

        match &edge.kind {
            EdgeKind::Line => {
                let dx = bx - ax;
                let dy = by - ay;
                Some((dx * dx + dy * dy).sqrt())
            }
            EdgeKind::Cubic { ha, hb, .. } => {
                let cubic = CubicBezier {
                    p0: Vec2 { x: ax, y: ay },
                    p1: Vec2 { x: ax + ha.x, y: ay + ha.y },
                    p2: Vec2 { x: bx + hb.x, y: by + hb.y },
                    p3: Vec2 { x: bx, y: by },
                };
                Some(cubic.arc_length(0.5))
            }
            EdgeKind::Polyline { points } => {
                let mut length = 0.0;
                let mut prev = (ax, ay);
                for p in points {
                    let dx = p.x - prev.0;
                    let dy = p.y - prev.1;
                    length += (dx * dx + dy * dy).sqrt();
                    prev = (p.x, p.y);
                }
                let dx = bx - prev.0;
                let dy = by - prev.1;
                length += (dx * dx + dy * dy).sqrt();
                Some(length)
            }
        }
    }

    /// Get a point at a specific distance along a path.
    /// Returns position and tangent angle, or None if distance is out of range.
    pub fn point_on_path(&self, edge_ids: &[u32], distance: f32) -> Option<PathPoint> {
        if edge_ids.is_empty() {
            return None;
        }

        let mut remaining = distance;

        for &eid in edge_ids {
            let edge_len = self.edge_length(eid)?;

            if remaining <= edge_len {
                // Point is on this edge
                let t = if edge_len > 0.0 {
                    remaining / edge_len
                } else {
                    0.0
                };
                return self.point_on_edge(eid, t);
            }

            remaining -= edge_len;
        }

        // Distance exceeds path length - return endpoint of last edge
        if let Some(&last_eid) = edge_ids.last() {
            return self.point_on_edge(last_eid, 1.0);
        }

        None
    }

    /// Get a point at parameter t (0-1) along a single edge.
    pub fn point_on_edge(&self, edge_id: u32, t: f32) -> Option<PathPoint> {
        let edge = self.edges.get(edge_id as usize)?.as_ref()?;
        let (ax, ay) = self.get_node(edge.a)?;
        let (bx, by) = self.get_node(edge.b)?;

        let t = t.clamp(0.0, 1.0);

        match &edge.kind {
            EdgeKind::Line => {
                let x = ax + (bx - ax) * t;
                let y = ay + (by - ay) * t;
                let angle = (by - ay).atan2(bx - ax);
                Some(PathPoint { x, y, angle })
            }
            EdgeKind::Cubic { ha, hb, .. } => {
                let cubic = CubicBezier {
                    p0: Vec2 { x: ax, y: ay },
                    p1: Vec2 { x: ax + ha.x, y: ay + ha.y },
                    p2: Vec2 { x: bx + hb.x, y: by + hb.y },
                    p3: Vec2 { x: bx, y: by },
                };
                let pos = cubic.eval(t);
                let tangent = cubic.tangent(t);
                let angle = tangent.y.atan2(tangent.x);
                Some(PathPoint {
                    x: pos.x,
                    y: pos.y,
                    angle,
                })
            }
            EdgeKind::Polyline { points } => {
                // Calculate total length and find the segment
                let mut total_len = 0.0;
                let mut prev = (ax, ay);
                let mut segments: Vec<((f32, f32), (f32, f32), f32)> = Vec::new();

                for p in points {
                    let dx = p.x - prev.0;
                    let dy = p.y - prev.1;
                    let len = (dx * dx + dy * dy).sqrt();
                    segments.push((prev, (p.x, p.y), len));
                    total_len += len;
                    prev = (p.x, p.y);
                }
                let dx = bx - prev.0;
                let dy = by - prev.1;
                let len = (dx * dx + dy * dy).sqrt();
                segments.push((prev, (bx, by), len));
                total_len += len;

                let target_dist = t * total_len;
                let mut accum = 0.0;

                for (start, end, seg_len) in segments {
                    if accum + seg_len >= target_dist || seg_len == 0.0 {
                        let local_t = if seg_len > 0.0 {
                            (target_dist - accum) / seg_len
                        } else {
                            0.0
                        };
                        let x = start.0 + (end.0 - start.0) * local_t;
                        let y = start.1 + (end.1 - start.1) * local_t;
                        let angle = (end.1 - start.1).atan2(end.0 - start.0);
                        return Some(PathPoint { x, y, angle });
                    }
                    accum += seg_len;
                }

                // Fallback to endpoint
                let angle = (by - prev.0).atan2(bx - prev.1);
                Some(PathPoint { x: bx, y: by, angle })
            }
        }
    }

    /// Sample positions for text characters along a path.
    /// Returns a position and angle for each character based on widths.
    ///
    /// # Arguments
    /// * `edge_ids` - Edge IDs forming the path
    /// * `char_widths` - Width of each character
    /// * `start_offset` - Starting position as fraction (0.0 to 1.0) of total path length
    ///
    /// # Returns
    /// Vec of (x, y, angle) tuples for each character's baseline position
    pub fn sample_text_positions(
        &self,
        edge_ids: &[u32],
        char_widths: &[f32],
        start_offset: f32,
    ) -> Vec<PathPoint> {
        let total_length = self.path_length(edge_ids);
        if total_length <= 0.0 || char_widths.is_empty() {
            return Vec::new();
        }

        let start_dist = start_offset.clamp(0.0, 1.0) * total_length;
        let mut positions = Vec::with_capacity(char_widths.len());
        let mut current_dist = start_dist;

        for &width in char_widths {
            // Place character at current position
            if let Some(point) = self.point_on_path(edge_ids, current_dist) {
                positions.push(point);
            }
            // Advance by character width (plus any letter spacing handled externally)
            current_dist += width;
        }

        positions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_length() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(3.0, 4.0);
        let e = g.add_edge(n0, n1).unwrap();

        let len = g.edge_length(e).unwrap();
        assert!((len - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_path_length() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(10.0, 0.0);
        let n2 = g.add_node(10.0, 10.0);
        let e0 = g.add_edge(n0, n1).unwrap();
        let e1 = g.add_edge(n1, n2).unwrap();

        let len = g.path_length(&[e0, e1]);
        assert!((len - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_point_on_line() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(10.0, 0.0);
        let e = g.add_edge(n0, n1).unwrap();

        let p = g.point_on_edge(e, 0.5).unwrap();
        assert!((p.x - 5.0).abs() < 0.001);
        assert!((p.y - 0.0).abs() < 0.001);
        assert!((p.angle - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_point_on_path() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(10.0, 0.0);
        let n2 = g.add_node(10.0, 10.0);
        let e0 = g.add_edge(n0, n1).unwrap();
        let e1 = g.add_edge(n1, n2).unwrap();

        // Middle of first edge
        let p1 = g.point_on_path(&[e0, e1], 5.0).unwrap();
        assert!((p1.x - 5.0).abs() < 0.001);
        assert!((p1.y - 0.0).abs() < 0.001);

        // Middle of second edge
        let p2 = g.point_on_path(&[e0, e1], 15.0).unwrap();
        assert!((p2.x - 10.0).abs() < 0.001);
        assert!((p2.y - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_sample_text_positions() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(100.0, 0.0);
        let e = g.add_edge(n0, n1).unwrap();

        // Three characters of width 10 each
        let widths = vec![10.0, 10.0, 10.0];
        let positions = g.sample_text_positions(&[e], &widths, 0.0);

        assert_eq!(positions.len(), 3);
        assert!((positions[0].x - 0.0).abs() < 0.001);
        assert!((positions[1].x - 10.0).abs() < 0.001);
        assert!((positions[2].x - 20.0).abs() < 0.001);
    }
}
