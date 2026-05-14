use crate::geometry::limits;
use crate::model::{EdgeKind, FontStyle, TextAlign};
use crate::Graph;
use std::collections::HashMap;

pub fn to_svg_paths_impl(g: &Graph) -> Vec<String> {
    let mut paths = Vec::new();
    for e in g.edges.iter() {
        if let Some(e) = e {
            let a = if let Some(n) = g.nodes.get(e.a as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            let b = if let Some(n) = g.nodes.get(e.b as usize).and_then(|n| *n) {
                n
            } else {
                continue;
            };
            match &e.kind {
                EdgeKind::Line => paths.push(format!("M {} {} L {} {}", a.x, a.y, b.x, b.y)),
                EdgeKind::Cubic { ha, hb, .. } => {
                    let p1x = a.x + ha.x;
                    let p1y = a.y + ha.y;
                    let p2x = b.x + hb.x;
                    let p2y = b.y + hb.y;
                    paths.push(format!(
                        "M {} {} C {} {}, {} {}, {} {}",
                        a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y
                    ));
                }
                EdgeKind::Polyline { points } => {
                    let mut d = format!("M {} {}", a.x, a.y);
                    for p in points {
                        d.push_str(&format!(" L {} {}", p.x, p.y));
                    }
                    d.push_str(&format!(" L {} {}", b.x, b.y));
                    paths.push(d);
                }
            }
        }
    }
    paths
}

/// Generate a complete SVG document string with paths, styles, and text
pub fn to_svg_document_impl(g: &Graph) -> String {
    let mut elements = Vec::new();
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    // Helper to update bounding box
    let mut update_bbox = |x: f32, y: f32| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    };

    // Export edges as path elements
    for e in g.edges.iter() {
        if let Some(e) = e {
            let a = match g.nodes.get(e.a as usize).and_then(|n| *n) {
                Some(n) => n,
                None => continue,
            };
            let b = match g.nodes.get(e.b as usize).and_then(|n| *n) {
                Some(n) => n,
                None => continue,
            };

            update_bbox(a.x, a.y);
            update_bbox(b.x, b.y);

            // Build path d attribute
            let d = match &e.kind {
                EdgeKind::Line => format!("M {} {} L {} {}", a.x, a.y, b.x, b.y),
                EdgeKind::Cubic { ha, hb, .. } => {
                    let p1x = a.x + ha.x;
                    let p1y = a.y + ha.y;
                    let p2x = b.x + hb.x;
                    let p2y = b.y + hb.y;
                    update_bbox(p1x, p1y);
                    update_bbox(p2x, p2y);
                    format!(
                        "M {} {} C {} {}, {} {}, {} {}",
                        a.x, a.y, p1x, p1y, p2x, p2y, b.x, b.y
                    )
                }
                EdgeKind::Polyline { points } => {
                    let mut d = format!("M {} {}", a.x, a.y);
                    for p in points {
                        update_bbox(p.x, p.y);
                        d.push_str(&format!(" L {} {}", p.x, p.y));
                    }
                    d.push_str(&format!(" L {} {}", b.x, b.y));
                    d
                }
            };

            // Build style attributes
            let stroke_color = e
                .stroke
                .as_ref()
                .map(|c| {
                    if c.a == 255 {
                        format!("rgb({},{},{})", c.r, c.g, c.b)
                    } else {
                        format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a as f32 / 255.0)
                    }
                })
                .unwrap_or_else(|| "black".to_string());
            let stroke_width = e.stroke_width;

            elements.push(format!(
                r#"  <path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-linecap="round" stroke-linejoin="round"/>"#,
                d, stroke_color, stroke_width
            ));
        }
    }

    // Export text elements
    for text in g.texts.iter().filter_map(|t| t.as_ref()) {
        update_bbox(text.position.x, text.position.y);
        // Estimate text extent for bbox (rough approximation)
        let est_width = text.content.len() as f32 * text.style.font_size * 0.6;
        update_bbox(text.position.x + est_width, text.position.y + text.style.font_size);

        // Build text-anchor from align
        let text_anchor = match text.align {
            TextAlign::Left => "start",
            TextAlign::Center => "middle",
            TextAlign::Right => "end",
        };

        // Build font-style
        let font_style = match text.style.font_style {
            FontStyle::Normal => "normal",
            FontStyle::Italic => "italic",
            FontStyle::Oblique => "oblique",
        };

        // Build fill color
        let fill = text
            .style
            .fill_color
            .as_ref()
            .map(|c| {
                if c.a == 255 {
                    format!("rgb({},{},{})", c.r, c.g, c.b)
                } else {
                    format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a as f32 / 255.0)
                }
            })
            .unwrap_or_else(|| "black".to_string());

        // Build optional stroke
        let stroke_attrs = match &text.style.stroke_color {
            Some(c) if text.style.stroke_width > 0.0 => {
                let stroke_col = if c.a == 255 {
                    format!("rgb({},{},{})", c.r, c.g, c.b)
                } else {
                    format!("rgba({},{},{},{})", c.r, c.g, c.b, c.a as f32 / 255.0)
                };
                format!(
                    r#" stroke="{}" stroke-width="{}""#,
                    stroke_col, text.style.stroke_width
                )
            }
            _ => String::new(),
        };

        // Build transform for rotation (convert radians to degrees)
        let transform = if text.rotation.abs() > 1e-6 {
            let degrees = text.rotation.to_degrees();
            format!(
                r#" transform="rotate({:.2}, {}, {})""#,
                degrees, text.position.x, text.position.y
            )
        } else {
            String::new()
        };

        // Build letter-spacing if non-zero
        let letter_spacing = if text.style.letter_spacing.abs() > 1e-6 {
            format!(r#" letter-spacing="{}em""#, text.style.letter_spacing)
        } else {
            String::new()
        };

        // Escape XML content
        let content = escape_xml(&text.content);

        elements.push(format!(
            r#"  <text x="{}" y="{}" font-family="{}" font-size="{}" font-weight="{}" font-style="{}" text-anchor="{}" fill="{}"{}{}{}>{}</text>"#,
            text.position.x,
            text.position.y,
            escape_xml_attr(&text.style.font_family),
            text.style.font_size,
            text.style.font_weight,
            font_style,
            text_anchor,
            fill,
            stroke_attrs,
            transform,
            letter_spacing,
            content
        ));
    }

    // Calculate viewBox with padding
    let padding = 10.0;
    if min_x > max_x || min_y > max_y {
        // Empty document
        return r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"></svg>"#
            .to_string();
    }

    let vb_x = min_x - padding;
    let vb_y = min_y - padding;
    let vb_w = (max_x - min_x) + padding * 2.0;
    let vb_h = (max_y - min_y) + padding * 2.0;

    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}">
{}
</svg>"#,
        vb_x, vb_y, vb_w, vb_h,
        elements.join("\n")
    )
}

/// Escape special XML characters in content
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape special XML characters in attribute values (includes quotes)
fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn add_svg_path_impl(g: &mut Graph, d: &str, rgba: Option<(u8, u8, u8, u8, f32)>) -> u32 {
    if d.len() > limits::MAX_SVG_TOKENS {
        return 0;
    }
    let mut i = 0usize;
    let bytes = d.as_bytes();
    let mut cur = (0.0f32, 0.0f32);
    let mut start_sub = (0.0f32, 0.0f32);
    let mut last_cmd = b'M';
    let mut edges_added = 0u32;
    let mut cmd_count = 0usize;
    let mut subpaths = 0usize;
    let mut segs = 0usize;
    let mut node_cache: HashMap<(i32, i32), u32> = HashMap::new();
    let q = |x: f32, y: f32| ((x * 100.0).round() as i32, (y * 100.0).round() as i32);
    let mut get_node = |x: f32, y: f32, this: &mut Graph| -> u32 {
        let key = q(x, y);
        if let Some(&id) = node_cache.get(&key) {
            return id;
        }
        let id = this.add_node(x, y);
        node_cache.insert(key, id);
        id
    };
    fn skip_ws(bytes: &[u8], i: &mut usize) {
        while *i < bytes.len() {
            let c = bytes[*i];
            if c == b' ' || c == b'\n' || c == b'\t' || c == b',' {
                *i += 1;
            } else {
                break;
            }
        }
    }
    fn parse_num(bytes: &[u8], i: &mut usize) -> Option<f32> {
        skip_ws(bytes, i);
        let start = *i;
        let mut had = false;
        while *i < bytes.len() {
            let c = bytes[*i];
            if (c as char).is_ascii_digit()
                || c == b'.'
                || c == b'-'
                || c == b'+'
                || c == b'e'
                || c == b'E'
            {
                had = true;
                *i += 1;
            } else {
                break;
            }
        }
        if !had {
            return None;
        }
        let s = std::str::from_utf8(&bytes[start..*i]).ok()?;
        let v = s.parse::<f32>().ok()?;
        if limits::in_coord_bounds(v) {
            Some(v)
        } else {
            None
        }
    }
    while i < bytes.len() {
        skip_ws(bytes, &mut i);
        if i >= bytes.len() {
            break;
        }
        let c = bytes[i];
        let is_cmd = matches!(c, b'M' | b'm' | b'L' | b'l' | b'C' | b'c' | b'Z' | b'z');
        let cmd = if is_cmd {
            i += 1;
            c
        } else {
            last_cmd
        };
        last_cmd = cmd;
        match cmd {
            b'M' | b'm' => {
                cmd_count += 1;
                if cmd_count > limits::MAX_SVG_COMMANDS {
                    return edges_added;
                }
                subpaths += 1;
                if subpaths > limits::MAX_SVG_SUBPATHS {
                    return edges_added;
                }
                let mut x = parse_num(bytes, &mut i).unwrap_or(cur.0);
                let mut y = parse_num(bytes, &mut i).unwrap_or(cur.1);
                if cmd == b'm' {
                    x += cur.0;
                    y += cur.1;
                }
                if !limits::in_coord_bounds(x) || !limits::in_coord_bounds(y) {
                    return edges_added;
                }
                cur = (x, y);
                start_sub = cur;
                loop {
                    skip_ws(bytes, &mut i);
                    if i >= bytes.len() {
                        break;
                    }
                    let peek = bytes[i];
                    if matches!(peek, b'M' | b'm' | b'L' | b'l' | b'C' | b'c' | b'Z' | b'z') {
                        break;
                    }
                    let mut nx = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut ny = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    if cmd == b'm' {
                        nx += cur.0;
                        ny += cur.1;
                    }
                    if !limits::in_coord_bounds(nx) || !limits::in_coord_bounds(ny) {
                        return edges_added;
                    }
                    let a_id = get_node(cur.0, cur.1, g);
                    let b_id = get_node(nx, ny, g);
                    if a_id != b_id {
                        if let Some(eid) = g.add_edge(a_id, b_id) {
                            if let Some((r, gg, b, aa, w)) = rgba {
                                if limits::in_width_bounds(w) {
                                    g.set_edge_style(eid, r, gg, b, aa, w);
                                }
                            }
                            edges_added += 1;
                            segs += 1;
                            if segs > limits::MAX_SVG_SEGMENTS {
                                return edges_added;
                            }
                        }
                    }
                    cur = (nx, ny);
                }
            }
            b'L' | b'l' => {
                cmd_count += 1;
                if cmd_count > limits::MAX_SVG_COMMANDS {
                    return edges_added;
                }
                loop {
                    let mut x = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut y = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    if cmd == b'l' {
                        x += cur.0;
                        y += cur.1;
                    }
                    if !limits::in_coord_bounds(x) || !limits::in_coord_bounds(y) {
                        return edges_added;
                    }
                    let a_id = get_node(cur.0, cur.1, g);
                    let b_id = get_node(x, y, g);
                    if a_id != b_id {
                        if let Some(eid) = g.add_edge(a_id, b_id) {
                            if let Some((r, gg, b, aa, w)) = rgba {
                                if limits::in_width_bounds(w) {
                                    g.set_edge_style(eid, r, gg, b, aa, w);
                                }
                            }
                            edges_added += 1;
                            segs += 1;
                            if segs > limits::MAX_SVG_SEGMENTS {
                                return edges_added;
                            }
                        }
                    }
                    cur = (x, y);
                    skip_ws(bytes, &mut i);
                    if i >= bytes.len() {
                        break;
                    }
                    let peek = bytes[i];
                    if matches!(peek, b'M' | b'm' | b'L' | b'l' | b'C' | b'c' | b'Z' | b'z') {
                        break;
                    }
                }
            }
            b'C' | b'c' => {
                cmd_count += 1;
                if cmd_count > limits::MAX_SVG_COMMANDS {
                    return edges_added;
                }
                loop {
                    let mut x1 = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut y1 = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut x2 = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut y2 = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut x = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    let mut y = match parse_num(bytes, &mut i) {
                        Some(v) => v,
                        None => break,
                    };
                    if cmd == b'c' {
                        x1 += cur.0;
                        y1 += cur.1;
                        x2 += cur.0;
                        y2 += cur.1;
                        x += cur.0;
                        y += cur.1;
                    }
                    if !limits::in_coord_bounds(x1)
                        || !limits::in_coord_bounds(y1)
                        || !limits::in_coord_bounds(x2)
                        || !limits::in_coord_bounds(y2)
                        || !limits::in_coord_bounds(x)
                        || !limits::in_coord_bounds(y)
                    {
                        return edges_added;
                    }
                    let a_id = get_node(cur.0, cur.1, g);
                    let b_id = get_node(x, y, g);
                    if a_id != b_id {
                        if let Some(eid) = g.add_edge(a_id, b_id) {
                            g.set_edge_cubic(eid, x1, y1, x2, y2);
                            if let Some((r, gg, b, aa, w)) = rgba {
                                if limits::in_width_bounds(w) {
                                    g.set_edge_style(eid, r, gg, b, aa, w);
                                }
                            }
                            edges_added += 1;
                            segs += 1;
                            if segs > limits::MAX_SVG_SEGMENTS {
                                return edges_added;
                            }
                        }
                    }
                    cur = (x, y);
                    skip_ws(bytes, &mut i);
                    if i >= bytes.len() {
                        break;
                    }
                    let peek = bytes[i];
                    if matches!(peek, b'M' | b'm' | b'L' | b'l' | b'C' | b'c' | b'Z' | b'z') {
                        break;
                    }
                }
            }
            b'Z' | b'z' => {
                cmd_count += 1;
                if cmd_count > limits::MAX_SVG_COMMANDS {
                    return edges_added;
                }
                let a_id = get_node(cur.0, cur.1, g);
                let b_id = get_node(start_sub.0, start_sub.1, g);
                if a_id != b_id {
                    if let Some(eid) = g.add_edge(a_id, b_id) {
                        if let Some((r, gg, b, aa, w)) = rgba {
                            if limits::in_width_bounds(w) {
                                g.set_edge_style(eid, r, gg, b, aa, w);
                            }
                        }
                        edges_added += 1;
                        segs += 1;
                        if segs > limits::MAX_SVG_SEGMENTS {
                            return edges_added;
                        }
                    }
                }
                cur = start_sub;
            }
            _ => {}
        }
    }
    if edges_added > 0 { /* bump handled by add_edge */ }
    edges_added
}
