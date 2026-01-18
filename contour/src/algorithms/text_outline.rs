//! Text to outlines conversion algorithm.
//!
//! Converts glyph outline data (received from JavaScript font library) into vector paths.
//! The glyphs are transformed according to the text element's style and position.

use crate::model::{
    EdgeKind, GlyphOutline, GlyphPath, HandleMode, PathCommand, TextElement, TextType, Vec2,
};
use crate::Graph;

/// Result of text-to-outlines conversion
#[derive(Debug, Clone)]
pub struct TextOutlineResult {
    /// Shape IDs created for each glyph
    pub shapes: Vec<u32>,
    /// Node IDs created
    pub nodes: Vec<u32>,
    /// Edge IDs created
    pub edges: Vec<u32>,
}

impl Graph {
    /// Convert a text element to vector outlines using provided glyph data.
    ///
    /// # Arguments
    /// * `text_id` - ID of the text element to convert
    /// * `glyphs` - Glyph outlines for each character (from JavaScript font library)
    ///
    /// # Returns
    /// TextOutlineResult with created shapes, nodes, and edges, or None if text not found.
    pub fn text_to_outlines(
        &mut self,
        text_id: u32,
        glyphs: &[GlyphOutline],
    ) -> Option<TextOutlineResult> {
        let text = self.get_text(text_id)?.clone();

        let mut result = TextOutlineResult {
            shapes: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };

        // Calculate scale factor from font units to pixels
        let scale = text.style.font_size / 1000.0; // Assume 1000 units per em

        // Get base position
        let (base_x, base_y) = match &text.text_type {
            TextType::Label => (text.position.x, text.position.y),
            TextType::Box { .. } => (text.position.x, text.position.y),
            TextType::OnPath { .. } => {
                // For text on path, we'd need to sample positions along the path
                // For now, fall back to position
                (text.position.x, text.position.y)
            }
        };

        // Track current X position for advancing through glyphs
        let mut current_x = base_x;
        let letter_spacing_px = text.style.letter_spacing * text.style.font_size;

        // Process each glyph
        for glyph in glyphs {
            // Process each contour in the glyph
            for path in &glyph.paths {
                let contour_result = self.add_glyph_contour(
                    path,
                    current_x,
                    base_y,
                    scale,
                    text.rotation,
                );

                if let Some((shape_id, nodes, edges)) = contour_result {
                    result.shapes.push(shape_id);
                    result.nodes.extend(nodes);
                    result.edges.extend(edges);
                }
            }

            // Advance position
            current_x += glyph.advance_width * scale + letter_spacing_px;
        }

        Some(result)
    }

    /// Add a single glyph contour to the graph.
    /// Returns (shape_id, node_ids, edge_ids) or None if contour is invalid.
    fn add_glyph_contour(
        &mut self,
        path: &GlyphPath,
        offset_x: f32,
        offset_y: f32,
        scale: f32,
        rotation: f32,
    ) -> Option<(u32, Vec<u32>, Vec<u32>)> {
        if path.commands.is_empty() {
            return None;
        }

        let (cos_r, sin_r) = (rotation.cos(), rotation.sin());

        // Transform a point with scale and rotation
        let transform = |x: f32, y: f32| -> (f32, f32) {
            let sx = x * scale;
            let sy = -y * scale; // Flip Y (font coords are Y-up)
            let rx = sx * cos_r - sy * sin_r + offset_x;
            let ry = sx * sin_r + sy * cos_r + offset_y;
            (rx, ry)
        };

        let mut nodes: Vec<u32> = Vec::new();
        let mut edges: Vec<u32> = Vec::new();
        let mut current_pos = (0.0f32, 0.0f32);
        let mut start_node: Option<u32> = None;
        let mut prev_node: Option<u32> = None;

        for cmd in &path.commands {
            match cmd {
                PathCommand::MoveTo(x, y) => {
                    let (tx, ty) = transform(*x, *y);
                    let node_id = self.add_node(tx, ty);
                    nodes.push(node_id);
                    start_node = Some(node_id);
                    prev_node = Some(node_id);
                    current_pos = (*x, *y);
                }
                PathCommand::LineTo(x, y) => {
                    let (tx, ty) = transform(*x, *y);
                    let node_id = self.add_node(tx, ty);
                    nodes.push(node_id);

                    if let Some(prev) = prev_node {
                        if let Some(edge_id) = self.add_edge(prev, node_id) {
                            edges.push(edge_id);
                        }
                    }

                    prev_node = Some(node_id);
                    current_pos = (*x, *y);
                }
                PathCommand::QuadTo(cx, cy, x, y) => {
                    // Convert quadratic to cubic bezier
                    let (p0x, p0y) = current_pos;
                    let cp1x = p0x + (cx - p0x) * 2.0 / 3.0;
                    let cp1y = p0y + (cy - p0y) * 2.0 / 3.0;
                    let cp2x = *x + (cx - x) * 2.0 / 3.0;
                    let cp2y = *y + (cy - y) * 2.0 / 3.0;

                    let (tx, ty) = transform(*x, *y);
                    let node_id = self.add_node(tx, ty);
                    nodes.push(node_id);

                    if let Some(prev) = prev_node {
                        if let Some(edge_id) = self.add_edge(prev, node_id) {
                            // Set cubic handles (as offsets from endpoints)
                            let (prev_x, prev_y) = self.get_node(prev)?;
                            let (tcp1x, tcp1y) = transform(cp1x, cp1y);
                            let (tcp2x, tcp2y) = transform(cp2x, cp2y);

                            let ha = Vec2 {
                                x: tcp1x - prev_x,
                                y: tcp1y - prev_y,
                            };
                            let hb = Vec2 {
                                x: tcp2x - tx,
                                y: tcp2y - ty,
                            };

                            self.set_edge_cubic_handles(edge_id, ha, hb);
                            edges.push(edge_id);
                        }
                    }

                    prev_node = Some(node_id);
                    current_pos = (*x, *y);
                }
                PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                    let (tx, ty) = transform(*x, *y);
                    let node_id = self.add_node(tx, ty);
                    nodes.push(node_id);

                    if let Some(prev) = prev_node {
                        if let Some(edge_id) = self.add_edge(prev, node_id) {
                            let (prev_x, prev_y) = self.get_node(prev)?;
                            let (tc1x, tc1y) = transform(*c1x, *c1y);
                            let (tc2x, tc2y) = transform(*c2x, *c2y);

                            let ha = Vec2 {
                                x: tc1x - prev_x,
                                y: tc1y - prev_y,
                            };
                            let hb = Vec2 {
                                x: tc2x - tx,
                                y: tc2y - ty,
                            };

                            self.set_edge_cubic_handles(edge_id, ha, hb);
                            edges.push(edge_id);
                        }
                    }

                    prev_node = Some(node_id);
                    current_pos = (*x, *y);
                }
                PathCommand::Close => {
                    // Close path by connecting back to start
                    if let (Some(prev), Some(start)) = (prev_node, start_node) {
                        if prev != start {
                            if let Some(edge_id) = self.add_edge(prev, start) {
                                edges.push(edge_id);
                            }
                        }
                    }
                    prev_node = start_node;
                }
            }
        }

        // Create shape from edges
        if !edges.is_empty() {
            if let Some(shape_id) = self.create_shape(&edges, true) {
                return Some((shape_id, nodes, edges));
            }
        }

        None
    }

    /// Internal helper to set cubic handles on an edge
    fn set_edge_cubic_handles(&mut self, edge_id: u32, ha: Vec2, hb: Vec2) {
        if let Some(Some(edge)) = self.edges.get_mut(edge_id as usize) {
            edge.kind = EdgeKind::Cubic {
                ha,
                hb,
                mode: HandleMode::Free,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GlyphOutline, GlyphPath, PathCommand};

    #[test]
    fn test_simple_glyph_to_outline() {
        let mut g = Graph::new();

        // Create a simple text label
        let text_id = g.add_text("A", 0.0, 0.0);

        // Create a simple triangle glyph (like a very basic 'A')
        let glyph = GlyphOutline {
            char: 'A',
            advance_width: 500.0,
            paths: vec![GlyphPath {
                commands: vec![
                    PathCommand::MoveTo(0.0, 0.0),
                    PathCommand::LineTo(250.0, 700.0),
                    PathCommand::LineTo(500.0, 0.0),
                    PathCommand::Close,
                ],
            }],
        };

        let result = g.text_to_outlines(text_id, &[glyph]);
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.shapes.len(), 1);
        assert_eq!(result.nodes.len(), 3);
        assert_eq!(result.edges.len(), 3);
    }

    #[test]
    fn test_curved_glyph() {
        let mut g = Graph::new();

        let text_id = g.add_text("O", 0.0, 0.0);

        // Create a simple curved glyph (approximating a circle)
        let glyph = GlyphOutline {
            char: 'O',
            advance_width: 600.0,
            paths: vec![GlyphPath {
                commands: vec![
                    PathCommand::MoveTo(300.0, 0.0),
                    PathCommand::CubicTo(465.0, 0.0, 600.0, 135.0, 600.0, 300.0),
                    PathCommand::CubicTo(600.0, 465.0, 465.0, 600.0, 300.0, 600.0),
                    PathCommand::CubicTo(135.0, 600.0, 0.0, 465.0, 0.0, 300.0),
                    PathCommand::CubicTo(0.0, 135.0, 135.0, 0.0, 300.0, 0.0),
                    PathCommand::Close,
                ],
            }],
        };

        let result = g.text_to_outlines(text_id, &[glyph]);
        assert!(result.is_some());

        let result = result.unwrap();
        assert_eq!(result.shapes.len(), 1);
        // 4 cubic segments + close = 5 edges, 5 nodes (last closes to first)
        assert!(result.edges.len() >= 4);
    }

    #[test]
    fn test_nonexistent_text() {
        let mut g = Graph::new();

        let glyph = GlyphOutline {
            char: 'X',
            advance_width: 500.0,
            paths: vec![],
        };

        let result = g.text_to_outlines(999, &[glyph]);
        assert!(result.is_none());
    }
}
