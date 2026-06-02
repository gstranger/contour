# Text Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add live font rendering, inline text editing (IDLE/SELECTED/EDITING states), cursor/selection, IME composition, and text-on-path rendering to the contour vector network engine and demo.

**Architecture:** The engine manages text state and layout; JS manages font loading, measurement, and Canvas 2D rendering. New engine methods (`measure_text`, `set_text_metrics`, `get_text_char_positions`, `get_text_hit`, `get_text_selection_bounds`, `sample_path_point`) follow the existing dirty/poll pattern (`get_dirty`). JS adds three modules: `FontManager` (opentype.js), `FontProvider` (measure → TextMetrics, outline → GlyphOutline[]), and `TextEditor` (keyboard, cursor, selection, IME).

**Tech Stack:** Rust (contour engine), wasm-bindgen, opentype.js, Canvas 2D.

**Spec:** `docs/superpowers/specs/2026-06-02-text-rendering-design.md`

---

## File Structure

| File | Role |
|------|------|
| `contour/src/model.rs` | Add `TextMetrics` struct, `metrics_ver` + `cached_metrics` + `cached_layout` fields to `TextElement` |
| `contour/src/lib.rs` | Add `measure_text()`, `set_text_metrics()`, `get_text_char_positions()`, `get_text_hit()`, `get_text_selection_bounds()` to `Graph`; modify `set_text_content()` and `set_text_style()` to bump `metrics_ver` |
| `contour/src/geometry/path_length.rs` | Add standalone `sample_path_point()` function (consistent with existing API) |
| `contour-wasm/src/api.rs` | WASM bindings for all new methods with `_res` strict variants |
| `contour-wasm/types.d.ts` | TypeScript declarations for new types and methods |
| `contour/tests/text_metrics.rs` (new) | Rust unit tests: cache invalidation, hit testing, char positions |
| `contour-wasm/tests/text_metrics_tests.rs` (new) | WASM integration tests: measure→set_metrics→measure cached pipeline |
| `web/font-provider.js` (new) | `FontManager` + `FontProvider` using opentype.js |
| `web/text-editor.js` (new) | `TextEditor` class: keyboard/mouse/IME, cursor, selection |
| `web/index.html` | Add opentype.js script, integrate text rendering + cursor + selection into canvas loop; add text state management |

---

### Task 1: Add `TextMetrics` to the data model and version tracking to `TextElement`

**Files:**
- Modify: `contour/src/model.rs` — add `TextMetrics` struct after `TextElement`, add version/cache fields
- Modify: `contour/src/lib.rs` — bump `metrics_ver` in `set_text_content()`, `set_text_style()`, `set_text_font()`, `set_text_font_weight()`, `set_text_font_style()`, `set_text_letter_spacing()`, `set_text_line_height()`, `set_text_fill_color()`, `set_text_stroke_color()`, `set_text_stroke_width()`, `set_text_position()`, `set_text_rotation()`, `set_text_align()`, `convert_text_*`, `set_text_box_*`, `set_text_path_*`

- [ ] **Step 1: Add `TextMetrics` to model.rs**

In `contour/src/model.rs`, add after the `TextElement` impl block (line ~508):

```rust
/// Position of one character within its text element.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TextCharPosition {
    /// X position relative to text element origin
    pub x: f32,
    /// Y position relative to text element origin (baseline)
    pub y: f32,
    /// Character width in pixels
    pub w: f32,
    /// Index into the content string (Unicode scalar index)
    pub char_index: u32,
}

/// Text measurement result from an external font provider.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TextMetrics {
    /// Width of each character in pixels, one entry per Unicode scalar
    pub char_widths: Vec<f32>,
    /// Computed line height in pixels
    pub line_height: f32,
    /// Distance from baseline to ascender top
    pub ascent: f32,
    /// Distance from baseline to descender bottom
    pub descent: f32,
    /// Total width of the rendered text
    pub total_width: f32,
}

/// Layout computed from TextMetrics + TextType.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TextCacheLayout {
    /// Per-character positions for cursor/selection rendering
    pub char_positions: Vec<TextCharPosition>,
}
```

- [ ] **Step 2: Add version + cache fields to `TextElement`**

In `contour/src/model.rs`, modify the `TextElement` struct (starting at line ~394) to add three fields at the end, before the closing `}`:

```rust
/// A text element in the document
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextElement {
    /// Unique identifier
    pub id: TextId,
    /// Text content
    pub content: String,
    /// Anchor position (x, y)
    pub position: Vec2,
    /// Rotation in radians
    pub rotation: f32,
    /// Text styling
    pub style: TextStyle,
    /// Horizontal text alignment
    pub align: TextAlign,
    /// Type of text (label, box, or on-path)
    pub text_type: TextType,
    /// Version bumped when content or style changes; used for metrics cache invalidation
    pub metrics_ver: u64,
    /// Cached metrics from the last measurement; cleared when metrics_ver changes
    #[serde(skip)]
    pub cached_metrics: Option<TextMetrics>,
    /// Cached per-character positions; recomputed after set_text_metrics
    #[serde(skip)]
    pub cached_layout: Option<TextCacheLayout>,
}
```

- [ ] **Step 3: Initialize new fields in TextElement constructors**

In `contour/src/model.rs`, update `new_label()`, `new_box()`, and `new_on_path()` to include the new fields. In each constructor, add after the last existing field:

```rust
            metrics_ver: 0,
            cached_metrics: None,
            cached_layout: None,
```

- [ ] **Step 4: Bump `metrics_ver` in all text-mutating methods**

In `contour/src/lib.rs`, find each text-mutating method and add `text.metrics_ver = text.metrics_ver.wrapping_add(1);` at the end, right before the `return true;`. The methods that need this are:

```
set_text_content       (line ~2889)
set_text_style         (line ~2969)
set_text_font          (line ~2978)
set_text_font_weight   (line ~2988)
set_text_font_style    (line ~2997)
set_text_fill_color    (line ~3007)
clear_text_fill_color  (line ~3016)
set_text_stroke_color  (line ~3025)
set_text_stroke_width  (line ~3034)
set_text_letter_spacing (line ~3043)
set_text_line_height   (line ~3052)
set_text_position      (line ~2897)
set_text_rotation      (line ~2904)
set_text_align         (line ~2957)
convert_text_to_box    (line ~3061)
convert_text_to_on_path (line ~3076)
convert_text_to_label  (line ~3102)
set_text_box_size      (line ~3107)
set_text_box_vertical_align (line ~3132)
set_text_box_overflow  (line ~3152)
set_text_path_offset   (line ~3166)
set_text_path_edges    (line ~3181)
```

Example for `set_text_content`:

```rust
    pub fn set_text_content(&mut self, id: TextId, content: &str) -> bool {
        if let Some(Some(text)) = self.texts.get_mut(id as usize) {
            text.content = content.to_string();
            text.metrics_ver = text.metrics_ver.wrapping_add(1);
            text.cached_metrics = None;
            text.cached_layout = None;
            return true;
        }
        false
    }
```

For `set_text_style`, add at end of success path:
```rust
            text.style = style;
            text.metrics_ver = text.metrics_ver.wrapping_add(1);
            text.cached_metrics = None;
            text.cached_layout = None;
```

Apply the same pattern to all 22 methods listed. Each adds the three lines:
```rust
            text.metrics_ver = text.metrics_ver.wrapping_add(1);
            text.cached_metrics = None;
            text.cached_layout = None;
```

- [ ] **Step 5: Verify compilation**

```bash
cd contour && cargo build -p contour 2>&1
```

Expected: compiles without errors (some dead-code warnings for the new structs are fine).

- [ ] **Step 6: Run existing text tests to confirm no regressions**

```bash
cargo test -p contour --test text 2>&1
```

Expected: `test result: ok. 10 passed; 0 failed;`

- [ ] **Step 7: Commit**

```bash
git add contour/src/model.rs contour/src/lib.rs
git commit -m "feat: add TextMetrics struct and metrics version tracking to TextElement"
```

---

### Task 2: Add `measure_text()` and `set_text_metrics()` to the engine

**Files:**
- Modify: `contour/src/lib.rs` — add the two methods to `Graph`
- Create: `contour/tests/text_metrics.rs` — unit tests

- [ ] **Step 1: Write the unit test**

Create `contour/tests/text_metrics.rs`:

```rust
use contour::Graph;
use contour::model::{TextMetrics, TextCacheLayout};

#[test]
fn test_measure_text_returns_needs_measure_for_new_text() {
    let mut g = Graph::new();
    let id = g.add_text("Hello", 0.0, 0.0);

    let result = g.measure_text(id);
    assert!(result.is_some());
    let needs = result.unwrap();
    assert!(needs.needs_measure, "new text should need measurement");
    assert_eq!(needs.content, "Hello");
}

#[test]
fn test_set_metrics_then_measure_returns_cached() {
    let mut g = Graph::new();
    let id = g.add_text("Hi", 0.0, 0.0);

    let metrics = TextMetrics {
        char_widths: vec![8.0, 5.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 13.0,
    };
    assert!(g.set_text_metrics(id, metrics));

    let result = g.measure_text(id);
    assert!(result.is_some());
    let cached = result.unwrap();
    assert!(!cached.needs_measure, "should be cached after set_text_metrics");
    assert_eq!(cached.total_width, 13.0);
}

#[test]
fn test_editing_content_invalidates_cache() {
    let mut g = Graph::new();
    let id = g.add_text("Hi", 0.0, 0.0);

    let metrics = TextMetrics {
        char_widths: vec![8.0, 5.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 13.0,
    };
    g.set_text_metrics(id, metrics);

    // Edit content
    g.set_text_content(id, "Hello");

    let result = g.measure_text(id);
    assert!(result.is_some());
    assert!(result.unwrap().needs_measure, "should need re-measure after edit");
}

#[test]
fn test_measure_text_returns_none_for_invalid_id() {
    let g = Graph::new();
    assert!(g.measure_text(999).is_none());
}

#[test]
fn test_set_text_metrics_returns_false_for_invalid_id() {
    let mut g = Graph::new();
    assert!(!g.set_text_metrics(999, TextMetrics::default()));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p contour --test text_metrics 2>&1
```

Expected: compilation errors — `measure_text` and `set_text_metrics` methods don't exist yet.

**Step 4a: Add `TextMeasureResult` and `TextCharPosition` to the model imports**

In `contour/src/lib.rs`, find the `use model::{` line (~line 30) and add the new types:

Change:
```rust
use model::{Color, ColorStop, DropShadow, Edge, EdgeKind, Effect, EffectId, EffectStack, FillRule, ...
```

Add at the end of the import list (before the closing `}`):
```rust
    TextCharPosition, TextCacheLayout, TextMeasureResult, TextMetrics,
```

The relevant part of the import should now include (among others):
```rust
    TextAlign, TextElement, TextId, TextOverflow, TextStyle, TextType,
    TextCharPosition, TextCacheLayout, TextMeasureResult, TextMetrics,
    Vec2, VerticalAlign,
```

- [ ] **Step 4b: Add the `TextMeasureResult` struct and methods to `lib.rs`**

In `contour/src/lib.rs`, add the return type struct near the other public types (around line 60, after `RegionFaceCache`):

```rust
/// Returned by `measure_text()`. Either cached metrics or a request for JS-side measurement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextMeasureResult {
    /// True if the caller should measure the text via a font provider
    pub needs_measure: bool,
    /// The text content to measure (valid when needs_measure is true, empty string otherwise)
    pub content: String,
    /// The text style (serialized; valid when needs_measure is true, defaults otherwise)
    pub style: TextStyle,
    /// Cached per-character widths (valid when needs_measure is false, empty vec otherwise)
    pub char_widths: Vec<f32>,
    /// Cached line height (valid when needs_measure is false, 0.0 otherwise)
    pub line_height: f32,
    /// Cached ascent (valid when needs_measure is false, 0.0 otherwise)
    pub ascent: f32,
    /// Cached descent (valid when needs_measure is false, 0.0 otherwise)
    pub descent: f32,
    /// Cached total width (valid when needs_measure is false, 0.0 otherwise)
    pub total_width: f32,
}
```

- [ ] **Step 4: Add the `measure_text()` method**

In `contour/src/lib.rs`, add to the text management `impl Graph` block (near line ~2800, after `add_text`):

```rust
    /// Poll whether a text element needs JS-side font measurement.
    /// Returns None if the text ID is invalid.
    /// Returns `needs_measure: true` if content/style changed since last measure.
    /// Returns `needs_measure: false` with cached metrics if still fresh.
    pub fn measure_text(&self, id: TextId) -> Option<TextMeasureResult> {
        let text = self.texts.get(id as usize)?.as_ref()?;
        if text.metrics_ver == 0 || text.cached_metrics.is_none() {
            Some(TextMeasureResult {
                needs_measure: true,
                content: text.content.clone(),
                style: text.style.clone(),
                char_widths: Vec::new(),
                line_height: 0.0,
                ascent: 0.0,
                descent: 0.0,
                total_width: 0.0,
            })
        } else {
            let m = text.cached_metrics.as_ref().unwrap();
            Some(TextMeasureResult {
                needs_measure: false,
                content: String::new(),
                style: TextStyle::default(),
                char_widths: m.char_widths.clone(),
                line_height: m.line_height,
                ascent: m.ascent,
                descent: m.descent,
                total_width: m.total_width,
            })
        }
    }
```

- [ ] **Step 5: Add the `set_text_metrics()` method**

In `contour/src/lib.rs`, add right after `measure_text()`:

```rust
    /// Store JS-measured text metrics and recompute per-character layout.
    /// Returns false if the text ID is invalid.
    pub fn set_text_metrics(&mut self, id: TextId, metrics: TextMetrics) -> bool {
        let text = match self.texts.get_mut(id as usize) {
            Some(Some(t)) => t,
            _ => return false,
        };

        // Compute character positions from metrics
        let mut char_positions = Vec::with_capacity(metrics.char_widths.len());
        let chars: Vec<char> = text.content.chars().collect();
        let letter_spacing_px = text.style.letter_spacing * text.style.font_size;

        // For labels: simple left-to-right positioning
        // For text boxes: use the layout engine
        match &text.text_type {
            TextType::Label => {
                let mut x = 0.0f32;
                let y = metrics.ascent; // baseline from top

                for (i, _ch) in chars.iter().enumerate() {
                    let w = metrics.char_widths.get(i).copied().unwrap_or(metrics.line_height * 0.5);
                    char_positions.push(TextCharPosition {
                        x,
                        y,
                        w,
                        char_index: i as u32,
                    });
                    x += w + letter_spacing_px;
                }
            }
            TextType::Box { width, height, vertical_align, overflow } => {
                use crate::algorithms::text_layout::layout_text_box;
                let layout = layout_text_box(
                    &text.content,
                    *width,
                    *height,
                    &text.style,
                    &metrics.char_widths,
                    text.align,
                    *vertical_align,
                );

                let mut char_idx = 0u32;
                for line in &layout.lines {
                    let mut x = line.x_offset;
                    for _ch in line.text.chars() {
                        let w = metrics.char_widths
                            .get(char_idx as usize)
                            .copied()
                            .unwrap_or(metrics.line_height * 0.5);
                        char_positions.push(TextCharPosition {
                            x,
                            y: line.y_offset,
                            w,
                            char_index: char_idx,
                        });
                        x += w + letter_spacing_px;
                        char_idx += 1;
                    }
                    // Skip newline character
                    char_idx += 1;
                }
            }
            TextType::OnPath { .. } => {
                // OnPath uses sample_text_positions at render time; no flat layout
                let mut x = 0.0f32;
                let y = metrics.ascent;
                for (i, _ch) in chars.iter().enumerate() {
                    let w = metrics.char_widths.get(i).copied().unwrap_or(metrics.line_height * 0.5);
                    char_positions.push(TextCharPosition {
                        x,
                        y,
                        w,
                        char_index: i as u32,
                    });
                    x += w + letter_spacing_px;
                }
            }
        }

        text.cached_metrics = Some(metrics.clone());
        text.cached_layout = Some(TextCacheLayout { char_positions });
        true
    }
```

- [ ] **Step 6: Add `get_text_char_positions()`, `get_text_hit()`, and `get_text_selection_bounds()`**

In `contour/src/lib.rs`, add after `set_text_metrics()`:

```rust
    /// Get per-character positions for cursor placement and hit testing.
    /// Returns None if the text ID is invalid or no layout has been computed.
    pub fn get_text_char_positions(&self, id: TextId) -> Option<Vec<TextCharPosition>> {
        let text = self.texts.get(id as usize)?.as_ref()?;
        let layout = text.cached_layout.as_ref()?;
        Some(layout.char_positions.clone())
    }

    /// Hit-test: which character is at world-coordinate (x, y)?
    /// Returns (char_index, line_index) or None.
    pub fn get_text_hit(&self, id: TextId, wx: f32, wy: f32) -> Option<(u32, u32)> {
        let text = self.texts.get(id as usize)?.as_ref()?;
        let layout = text.cached_layout.as_ref()?;
        let tx = wx - text.position.x;
        let ty = wy - text.position.y;

        let mut best_idx = None;
        let mut best_dist2 = f32::MAX;
        let mut line_idx = 0u32;

        for pos in &layout.char_positions {
            let cx = pos.x + pos.w * 0.5;
            let cy = pos.y;
            let dx = tx - cx;
            let dy = ty - cy;
            let d2 = dx * dx + dy * dy;
            if d2 < best_dist2 {
                best_dist2 = d2;
                best_idx = Some((pos.char_index, line_idx));
            }
        }

        best_idx
    }

    /// Get selection highlight rectangles for a character range.
    /// Returns None if the text ID is invalid or no layout exists.
    pub fn get_text_selection_bounds(&self, id: TextId, start: u32, end: u32) -> Option<Vec<[f32; 4]>> {
        let text = self.texts.get(id as usize)?.as_ref()?;
        let layout = text.cached_layout.as_ref()?;
        let start = start.min(end);
        let end = end.max(start);

        let line_height = text.cached_metrics.as_ref()
            .map(|m| m.line_height)
            .unwrap_or(text.style.font_size * text.style.line_height);

        let mut rects = Vec::new();
        for pos in &layout.char_positions {
            if pos.char_index >= start && pos.char_index < end {
                let x = text.position.x + pos.x;
                let y = text.position.y + pos.y - text.style.font_size * text.style.line_height;
                rects.push([x, y, pos.w, line_height]);
            }
        }
        Some(rects)
    }
```

- [ ] **Step 7: Run tests**

```bash
cargo test -p contour --test text_metrics 2>&1
```

Expected: 5 tests pass.

- [ ] **Step 8: Run all existing tests to confirm no regressions**

```bash
cargo test -p contour 2>&1
```

Expected: all 56+ tests pass.

- [ ] **Step 9: Commit**

```bash
git add contour/src/lib.rs contour/tests/text_metrics.rs
git commit -m "feat: add measure_text, set_text_metrics, get_text_char_positions, get_text_hit, get_text_selection_bounds"
```

---

### Task 3: Expose `sample_path_point()` for text-on-path

**Files:**
- Modify: `contour/src/geometry/path_length.rs` — add public standalone function
- Create/modify: tests in existing test file

- [ ] **Step 1: Add the public function**

In `contour/src/geometry/path_length.rs`, add after the `impl Graph` block (after line ~309):

```rust
/// Standalone: get a point at a specific distance along a path defined by edge IDs.
/// Returns position (x, y) and tangent angle in radians, or None if distance is out of range
/// or any edge is invalid.
///
/// This is a convenience wrapper around Graph::point_on_path for text-on-path rendering
/// where the caller may not have mutable access to the graph.
pub fn sample_path_point(g: &Graph, edge_ids: &[u32], distance: f32) -> Option<PathPoint> {
    g.point_on_path(edge_ids, distance)
}
```

- [ ] **Step 2: Write a unit test**

In `contour/src/geometry/path_length.rs`, at the end of the test module:

```rust
    #[test]
    fn test_standalone_sample_path_point() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(100.0, 0.0);
        let e = g.add_edge(n0, n1).unwrap();

        let p = sample_path_point(&g, &[e], 30.0).unwrap();
        assert!((p.x - 30.0).abs() < 0.001);
        assert!((p.y - 0.0).abs() < 0.001);
        assert!((p.angle - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_sample_path_point_past_end() {
        let mut g = Graph::new();
        let n0 = g.add_node(0.0, 0.0);
        let n1 = g.add_node(100.0, 0.0);
        let e = g.add_edge(n0, n1).unwrap();

        // Distance past end returns the endpoint
        let p = sample_path_point(&g, &[e], 200.0).unwrap();
        assert!((p.x - 100.0).abs() < 0.001);
    }
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p contour -- geometry::path_length 2>&1
```

Expected: all path length tests pass, including the two new ones.

- [ ] **Step 4: Commit**

```bash
git add contour/src/geometry/path_length.rs
git commit -m "feat: expose sample_path_point as standalone function for text-on-path"
```

---

### Task 4: WASM bindings for new text methods

**Files:**
- Modify: `contour-wasm/src/api.rs` — add WASM methods
- Modify: `contour-wasm/src/lib.rs` — forward internal methods
- Modify: `contour-wasm/types.d.ts` — TypeScript type declarations
- Create: `contour-wasm/tests/text_metrics_tests.rs` — integration tests

- [ ] **Step 1: Add forwarding methods in `lib.rs`**

In `contour-wasm/src/lib.rs`, add to the `impl Graph` block:

```rust
    pub fn rs_measure_text(&self, id: u32) -> Option<contour::TextMeasureResult> {
        self.inner.measure_text(id)
    }
    pub fn rs_set_text_metrics(&mut self, id: u32, metrics: contour::model::TextMetrics) -> bool {
        self.inner.set_text_metrics(id, metrics)
    }
    pub fn rs_get_text_char_positions(&self, id: u32) -> Option<Vec<contour::model::TextCharPosition>> {
        self.inner.get_text_char_positions(id)
    }
    pub fn rs_get_text_hit(&self, id: u32, x: f32, y: f32) -> Option<Vec<u32>> {
        self.inner.get_text_hit(id, x, y).map(|(ci, li)| vec![ci, li])
    }
    pub fn rs_get_text_selection_bounds(&self, id: u32, start: u32, end: u32) -> Option<Vec<[f32; 4]>> {
        self.inner.get_text_selection_bounds(id, start, end)
    }
    pub fn rs_sample_path_point(
        &self,
        edge_ids: &[u32],
        distance: f32,
    ) -> Option<contour::geometry::path_length::PathPoint> {
        contour::geometry::path_length::sample_path_point(&self.inner, edge_ids, distance)
    }
```

- [ ] **Step 2: Add WASM bindings in `api.rs`**

In `contour-wasm/src/api.rs`, add after the existing text management section (after the `add_text_res` methods, around line ~1700). Add these methods to the `#[wasm_bindgen] impl Graph` block:

```rust
    // ========== Path Length ==========

    /// Calculate the total length of a path defined by edge IDs.
    /// edge_ids: Uint32Array of edge IDs forming the path
    pub fn path_length(&self, edge_ids: &Uint32Array) -> f32 {
        let mut ids = vec![0u32; edge_ids.length() as usize];
        edge_ids.copy_to(&mut ids);
        self.inner.path_length(&ids)
    }

    // ========== Text Metrics & Layout ==========

    /// Poll whether a text element needs JS-side font measurement.
    /// Returns null if text ID is invalid.
    /// Returns { needs_measure: true, content, style } or { needs_measure: false, ...cached metrics }
    pub fn measure_text(&self, id: u32) -> JsValue {
        match self.inner.measure_text(id) {
            Some(result) => serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    pub fn measure_text_res(&self, id: u32) -> JsValue {
        match self.inner.measure_text(id) {
            Some(result) => error::ok(serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)),
            None => error::invalid_id("text", id),
        }
    }

    /// Store JS-measured text metrics. Returns false if text ID is invalid.
    /// metrics: { char_widths: number[], line_height: number, ascent: number, descent: number, total_width: number }
    pub fn set_text_metrics(&mut self, id: u32, metrics: &JsValue) -> bool {
        let m: contour::model::TextMetrics = match serde_wasm_bindgen::from_value(metrics.clone()) {
            Ok(m) => m,
            Err(_) => return false,
        };
        self.inner.set_text_metrics(id, m)
    }

    pub fn set_text_metrics_res(&mut self, id: u32, metrics: &JsValue) -> JsValue {
        let m: contour::model::TextMetrics = match serde_wasm_bindgen::from_value(metrics.clone()) {
            Ok(m) => m,
            Err(e) => return error::err("invalid_metrics", format!("{}", e), None),
        };
        if self.inner.set_text_metrics(id, m) {
            error::ok(JsValue::from_bool(true))
        } else {
            error::invalid_id("text", id)
        }
    }

    /// Get per-character positions for cursor and selection rendering.
    /// Returns [{x, y, w, char_index}, ...] or null.
    pub fn get_text_char_positions(&self, id: u32) -> JsValue {
        match self.inner.get_text_char_positions(id) {
            Some(positions) => serde_wasm_bindgen::to_value(&positions).unwrap_or(JsValue::NULL),
            None => JsValue::NULL,
        }
    }

    pub fn get_text_char_positions_res(&self, id: u32) -> JsValue {
        match self.inner.get_text_char_positions(id) {
            Some(positions) => error::ok(serde_wasm_bindgen::to_value(&positions).unwrap_or(JsValue::NULL)),
            None => error::invalid_id("text", id),
        }
    }

    /// Hit-test: which character is at world-coordinate (x, y)?
    /// Returns [char_index, line_index] or null.
    pub fn get_text_hit(&self, id: u32, x: f32, y: f32) -> JsValue {
        match self.inner.get_text_hit(id, x, y) {
            Some((ci, li)) => {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from_f64(ci as f64));
                arr.push(&JsValue::from_f64(li as f64));
                arr.into()
            }
            None => JsValue::NULL,
        }
    }

    pub fn get_text_hit_res(&self, id: u32, x: f32, y: f32) -> JsValue {
        if !x.is_finite() { return error::non_finite("x"); }
        if !y.is_finite() { return error::non_finite("y"); }
        match self.inner.get_text_hit(id, x, y) {
            Some((ci, li)) => {
                let arr = js_sys::Array::new();
                arr.push(&JsValue::from_f64(ci as f64));
                arr.push(&JsValue::from_f64(li as f64));
                error::ok(arr.into())
            }
            None => {
                if self.inner.get_text(id).is_some() {
                    error::err("no_layout", "text has not been measured yet", None)
                } else {
                    error::invalid_id("text", id)
                }
            }
        }
    }

    /// Get selection highlight rectangles for a character range.
    /// Returns [[x, y, w, h], ...] or null.
    pub fn get_text_selection_bounds(&self, id: u32, start: u32, end: u32) -> JsValue {
        match self.inner.get_text_selection_bounds(id, start, end) {
            Some(bounds) => {
                let arr = js_sys::Array::new();
                for b in bounds {
                    let sub = js_sys::Array::new();
                    sub.push(&JsValue::from_f64(b[0] as f64));
                    sub.push(&JsValue::from_f64(b[1] as f64));
                    sub.push(&JsValue::from_f64(b[2] as f64));
                    sub.push(&JsValue::from_f64(b[3] as f64));
                    arr.push(&sub);
                }
                arr.into()
            }
            None => JsValue::NULL,
        }
    }

    pub fn get_text_selection_bounds_res(&self, id: u32, start: u32, end: u32) -> JsValue {
        match self.inner.get_text_selection_bounds(id, start, end) {
            Some(bounds) => {
                let arr = js_sys::Array::new();
                for b in bounds {
                    let sub = js_sys::Array::new();
                    sub.push(&JsValue::from_f64(b[0] as f64));
                    sub.push(&JsValue::from_f64(b[1] as f64));
                    sub.push(&JsValue::from_f64(b[2] as f64));
                    sub.push(&JsValue::from_f64(b[3] as f64));
                    arr.push(&sub);
                }
                error::ok(arr.into())
            }
            None => {
                if self.inner.get_text(id).is_some() {
                    error::err("no_layout", "text has not been measured yet", None)
                } else {
                    error::invalid_id("text", id)
                }
            }
        }
    }

    /// Get a point and tangent angle at a specific distance along a chain of edges.
    /// Returns { x, y, angle } or null.
    /// edge_ids: Uint32Array of edge IDs forming the path
    /// distance: distance from start of path
    pub fn sample_path_point(&self, edge_ids: &Uint32Array, distance: f32) -> JsValue {
        let mut ids = vec![0u32; edge_ids.length() as usize];
        edge_ids.copy_to(&mut ids);
        match contour::geometry::path_length::sample_path_point(&self.inner, &ids, distance) {
            Some(pt) => {
                let obj = crate::interop::new_obj();
                crate::interop::set_kv(&obj, "x", &JsValue::from_f64(pt.x as f64));
                crate::interop::set_kv(&obj, "y", &JsValue::from_f64(pt.y as f64));
                crate::interop::set_kv(&obj, "angle", &JsValue::from_f64(pt.angle as f64));
                obj.into()
            }
            None => JsValue::NULL,
        }
    }

    pub fn sample_path_point_res(&self, edge_ids: &Uint32Array, distance: f32) -> JsValue {
        if !distance.is_finite() {
            return error::non_finite("distance");
        }
        if distance < 0.0 {
            return error::out_of_range("distance", 0.0, f32::INFINITY, distance);
        }
        let mut ids = vec![0u32; edge_ids.length() as usize];
        edge_ids.copy_to(&mut ids);
        for &id in &ids {
            if self.inner.edges.get(id as usize).and_then(|e| e.as_ref()).is_none() {
                return error::invalid_id("edge", id);
            }
        }
        match contour::geometry::path_length::sample_path_point(&self.inner, &ids, distance) {
            Some(pt) => {
                let obj = crate::interop::new_obj();
                crate::interop::set_kv(&obj, "x", &JsValue::from_f64(pt.x as f64));
                crate::interop::set_kv(&obj, "y", &JsValue::from_f64(pt.y as f64));
                crate::interop::set_kv(&obj, "angle", &JsValue::from_f64(pt.angle as f64));
                error::ok(obj.into())
            }
            None => error::err("out_of_range", "distance exceeds path length", None),
        }
    }
```

- [ ] **Step 3: Update TypeScript declarations**

In `contour-wasm/types.d.ts`, add after the existing `to_svg_paths_res` line (~line 60):

```typescript
  // Text metrics and layout
  measure_text(id: number): TextMeasureResult | null;
  measure_text_res(id: number): Result<TextMeasureResult>;
  set_text_metrics(id: number, metrics: TextMetrics): boolean;
  set_text_metrics_res(id: number, metrics: TextMetrics): Result<boolean>;
  get_text_char_positions(id: number): TextCharPosition[] | null;
  get_text_char_positions_res(id: number): Result<TextCharPosition[]>;
  get_text_hit(id: number, x: number, y: number): [number, number] | null;
  get_text_hit_res(id: number, x: number, y: number): Result<[number, number]>;
  get_text_selection_bounds(id: number, start: number, end: number): [number, number, number, number][] | null;
  get_text_selection_bounds_res(id: number, start: number, end: number): Result<[number, number, number, number][]>;
  sample_path_point(edge_ids: Uint32Array, distance: number): { x: number; y: number; angle: number } | null;
  sample_path_point_res(edge_ids: Uint32Array, distance: number): Result<{ x: number; y: number; angle: number }>;
```

And add at the bottom of the file before the final `}`:

```typescript
export interface TextMeasureResult {
  needs_measure: boolean;
  content: string;
  style: {
    font_family: string;
    font_size: number;
    font_weight: number;
    font_style: number;
    fill_color: { r: number; g: number; b: number; a: number } | null;
    stroke_color: { r: number; g: number; b: number; a: number } | null;
    stroke_width: number;
    letter_spacing: number;
    line_height: number;
  };
  char_widths: number[];
  line_height: number;
  ascent: number;
  descent: number;
  total_width: number;
}

export interface TextMetrics {
  char_widths: number[];
  line_height: number;
  ascent: number;
  descent: number;
  total_width: number;
}

export interface TextCharPosition {
  x: number;
  y: number;
  w: number;
  char_index: number;
}
```

- [ ] **Step 4: Write WASM integration test**

Create `contour-wasm/tests/text_metrics_tests.rs`:

```rust
use wasm_bindgen_test::*;
use contour_wasm::Graph;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_measure_text_new_text_needs_measure() {
    let g = Graph::new();
    let id = g.add_text("Hello", 0.0, 0.0);

    let result = g.measure_text(id);
    assert!(!result.is_null(), "should return value for valid text id");

    // measure_text returns a JS object; check it's not null
    // The engine's TextMeasureResult is serialized via serde-wasm-bindgen
}

#[wasm_bindgen_test]
fn test_set_metrics_then_measure_cached() {
    let mut g = Graph::new();
    let id = g.add_text("Hi", 0.0, 0.0);

    // Build metrics as a JS object via serde-wasm-bindgen
    let metrics = serde_wasm_bindgen::to_value(&serde_json::json!({
        "char_widths": [8.0, 5.0],
        "line_height": 19.2,
        "ascent": 14.0,
        "descent": 5.0,
        "total_width": 13.0
    })).unwrap();

    let ok = g.set_text_metrics(id, &metrics);
    assert!(ok, "set_text_metrics should succeed");

    let result = g.measure_text(id);
    assert!(!result.is_null(), "cached result should not be null");
}

#[wasm_bindgen_test]
fn test_measure_text_invalid_id_returns_null() {
    let g = Graph::new();
    let result = g.measure_text(999);
    assert!(result.is_null());
}

#[wasm_bindgen_test]
fn test_get_text_char_positions() {
    let mut g = Graph::new();
    let id = g.add_text("AB", 0.0, 0.0);

    let metrics = serde_wasm_bindgen::to_value(&serde_json::json!({
        "char_widths": [10.0, 10.0],
        "line_height": 19.2,
        "ascent": 14.0,
        "descent": 5.0,
        "total_width": 20.0
    })).unwrap();
    g.set_text_metrics(id, &metrics);

    let positions = g.get_text_char_positions(id);
    assert!(!positions.is_null(), "should return positions after measure");
}

#[wasm_bindgen_test]
fn test_sample_path_point() {
    let mut g = Graph::new();
    let n0 = g.add_node(0.0, 0.0);
    let n1 = g.add_node(100.0, 0.0);
    let e = g.add_edge(n0, n1).unwrap();

    let ids = js_sys::Uint32Array::from(&[e][..]);
    let pt = g.sample_path_point(&ids, 30.0);
    assert!(!pt.is_null(), "should return point at valid distance");
}
```

- [ ] **Step 5: Build and run WASM tests**

```bash
cd contour-wasm && wasm-pack test --node 2>&1
```

Expected: all tests pass (existing + new text metrics tests).

- [ ] **Step 6: Commit**

```bash
git add contour-wasm/src/api.rs contour-wasm/src/lib.rs contour-wasm/types.d.ts contour-wasm/tests/text_metrics_tests.rs
git commit -m "feat: WASM bindings for measure_text, set_text_metrics, char_positions, hit_test, selection_bounds, sample_path_point"
```

---

### Task 5: Build the JS FontManager and FontProvider

**Files:**
- Create: `web/font-provider.js`

- [ ] **Step 1: Create `web/font-provider.js`**

```javascript
// font-provider.js — font loading, measurement, and outline extraction using opentype.js
//
// Dependencies: opentype.js (loaded as <script src="https://unpkg.com/opentype.js@1.3.4/dist/opentype.min.js"></script>)

export class FontManager {
  constructor() {
    /** @type {Map<string, import('opentype.js').Font>} */
    this.fonts = new Map();

    /** @type {Map<string, Promise<import('opentype.js').Font>>} */
    this.loading = new Map();

    /** @type {string[]} */
    this.fallbackChain = ['sans-serif'];

    /**
     * Per-(font,size) glyph metrics cache.
     * Key: `family|size`
     * Value: Map<char, {width: number}>
     */
    this.glyphCache = new Map();
  }

  /**
   * Load a font from an ArrayBuffer.
   * @param {string} family
   * @param {ArrayBuffer} source
   * @param {number} [weight=400]
   * @param {'normal'|'italic'|'oblique'} [style='normal']
   */
  async load(family, source, weight = 400, style = 'normal') {
    const key = `${family}|${weight}|${style}`;
    if (this.fonts.has(key) || this.loading.has(key)) {
      return this.loading.get(key);
    }

    const promise = new Promise((resolve, reject) => {
      try {
        const font = opentype.parse(source);
        this.fonts.set(key, font);
        this.glyphCache.clear(); // invalidate glyph caches on new font
        resolve(font);
      } catch (e) {
        reject(e);
      }
    });

    this.loading.set(key, promise);
    return promise;
  }

  /**
   * Load a font from a URL.
   */
  async loadFromUrl(family, url, weight = 400, style = 'normal') {
    const resp = await fetch(url);
    const buf = await resp.arrayBuffer();
    return this.load(family, buf, weight, style);
  }

  unload(family) {
    for (const [key] of this.fonts) {
      if (key.startsWith(family + '|')) {
        this.fonts.delete(key);
      }
    }
    this.glyphCache.clear();
  }

  setFallbackChain(chain) {
    this.fallbackChain = chain;
    this.glyphCache.clear();
  }

  /**
   * Resolve a font family+weight+style to an opentype.Font.
   * Walks the fallback chain if exact match fails.
   * @returns {import('opentype.js').Font|null}
   */
  resolveFont(family, weight = 400, style = 'normal') {
    // 1. Exact match
    const exactKey = `${family}|${weight}|${style}`;
    if (this.fonts.has(exactKey)) {
      return this.fonts.get(exactKey);
    }

    // 2. Same family, closest weight
    let best = null;
    let bestDiff = Infinity;
    for (const [key, font] of this.fonts) {
      if (key.startsWith(family + '|')) {
        const parts = key.split('|');
        const w = parseInt(parts[1]) || 400;
        const diff = Math.abs(w - weight);
        if (diff < bestDiff) {
          bestDiff = diff;
          best = font;
        }
      }
    }
    if (best) return best;

    // 3. Walk fallback chain
    for (const fb of this.fallbackChain) {
      if (fb === family) continue; // already tried
      for (const [key, font] of this.fonts) {
        if (key.startsWith(fb + '|')) return font;
      }
    }

    return null;
  }

  /**
   * Get or compute glyph metrics for a character.
   * @param {import('opentype.js').Font} font
   * @param {number} fontSize
   * @param {string} char
   * @returns {{width: number}}
   */
  getGlyphMetrics(font, fontSize, char) {
    const cacheKey = `${font.names.fontFamily.en}|${font.tables.os2.usWeightClass}|${fontSize}`;
    if (!this.glyphCache.has(cacheKey)) {
      this.glyphCache.set(cacheKey, new Map());
    }
    const cache = this.glyphCache.get(cacheKey);
    if (cache.has(char)) {
      return cache.get(char);
    }

    const scale = fontSize / font.unitsPerEm;
    const glyphIndex = font.charToGlyphIndex(char);
    const glyph = font.glyphs.get(glyphIndex);
    const width = (glyph ? glyph.advanceWidth : font.unitsPerEm * 0.5) * scale;

    const metrics = { width };
    cache.set(char, metrics);
    return metrics;
  }
}

export class FontProvider {
  /**
   * @param {FontManager} fontManager
   */
  constructor(fontManager) {
    this.fontManager = fontManager;
  }

  /**
   * Measure text content with the given style.
   * @param {string} content
   * @param {{font_family: string, font_size: number, font_weight: number, font_style: number, letter_spacing: number, line_height: number}} style
   * @returns {{char_widths: number[], line_height: number, ascent: number, descent: number, total_width: number}}
   */
  measure(content, style) {
    const font = this.fontManager.resolveFont(
      style.font_family || 'sans-serif',
      style.font_weight || 400,
      ['normal', 'italic', 'oblique'][style.font_style] || 'normal'
    );

    const fontSize = style.font_size || 16;
    const letterSpacingPx = (style.letter_spacing || 0) * fontSize;

    // If no font available, return approximate metrics
    if (!font) {
      const charWidths = [...content].map(() => fontSize * 0.6);
      const totalWidth = charWidths.reduce((s, w) => s + w + letterSpacingPx, 0) - letterSpacingPx;
      return {
        char_widths: charWidths,
        line_height: fontSize * (style.line_height || 1.2),
        ascent: fontSize * 0.72,
        descent: fontSize * 0.28,
        total_width: Math.max(0, totalWidth),
      };
    }

    const scale = fontSize / font.unitsPerEm;
    const charWidths = [];
    let totalWidth = 0;

    for (const ch of [...content]) {
      if (ch === '\n') {
        charWidths.push(0);
        continue;
      }
      const metrics = this.fontManager.getGlyphMetrics(font, fontSize, ch);
      charWidths.push(metrics.width + letterSpacingPx);
      totalWidth += metrics.width + letterSpacingPx;
    }

    const lineHeight = fontSize * (style.line_height || 1.2);
    const ascent = font.ascender * scale;
    const descent = -font.descender * scale;

    return {
      char_widths: charWidths,
      line_height: lineHeight,
      ascent: ascent,
      descent: descent,
      total_width: totalWidth,
    };
  }

  /**
   * Generate glyph outlines for text-to-outlines conversion.
   * @param {string} content
   * @param {object} style
   * @returns {import('./types').GlyphOutline[]}
   */
  getOutlines(content, style) {
    const font = this.fontManager.resolveFont(
      style.font_family || 'sans-serif',
      style.font_weight || 400,
      ['normal', 'italic', 'oblique'][style.font_style] || 'normal'
    );
    if (!font) return [];

    const outlines = [];
    const size = style.font_size || 16;

    for (const ch of [...content]) {
      const glyphIndex = font.charToGlyphIndex(ch);
      const glyph = font.glyphs.get(glyphIndex);
      if (!glyph) {
        outlines.push({
          char: ch,
          advance_width: size * 0.5,
          paths: [],
        });
        continue;
      }

      const path = glyph.getPath(0, 0, size);
      const commands = [];

      // Parse opentype.js Path into our PathCommand format
      // opentype.js Path uses: moveTo, lineTo, quadTo, bezierCurveTo, close
      for (const cmd of path.commands) {
        switch (cmd.type) {
          case 'M':
            commands.push({ MoveTo: [cmd.x, cmd.y] });
            break;
          case 'L':
            commands.push({ LineTo: [cmd.x, cmd.y] });
            break;
          case 'Q':
            commands.push({ QuadTo: [cmd.x1, cmd.y1, cmd.x, cmd.y] });
            break;
          case 'C':
            commands.push({ CubicTo: [cmd.x1, cmd.y1, cmd.x2, cmd.y2, cmd.x, cmd.y] });
            break;
          case 'Z':
            commands.push('Close');
            break;
        }
      }

      outlines.push({
        char: ch,
        advance_width: glyph.advanceWidth * (size / font.unitsPerEm),
        paths: [{ commands }],
      });
    }

    return outlines;
  }
}
```

- [ ] **Step 2: Load opentype.js in the demo HTML**

In `web/index.html`, add before the existing `<script type="module">` tag:

```html
    <script src="https://unpkg.com/opentype.js@1.3.4/dist/opentype.min.js"></script>
```

- [ ] **Step 3: Commit**

```bash
git add web/font-provider.js web/index.html
git commit -m "feat: add FontManager and FontProvider using opentype.js"
```

---

### Task 6: Build the TextEditor for keyboard/cursor/selection/IME

**Files:**
- Create: `web/text-editor.js`

- [ ] **Step 1: Create `web/text-editor.js`**

```javascript
// text-editor.js — keyboard input, cursor, selection, and IME handling for inline text editing
//
// The TextEditor manages editing state in JS (cursor position, selection range,
// IME composition). It calls back into the engine (Graph) to persist committed
// text changes. The engine never sees partial IME strings.

export class TextEditor {
  /**
   * @param {import('./types').Graph} graph — the WASM Graph instance
   * @param {import('./font-provider').FontProvider} fontProvider
   */
  constructor(graph, fontProvider) {
    this.graph = graph;
    this.fontProvider = fontProvider;

    /** @type {number|null} */
    this.textId = null;

    /** @type {'idle'|'selected'|'editing'} */
    this.state = 'idle';

    /** @type {number} — char index within content */
    this.cursorPosition = 0;

    /** @type {number|null} — if set, selection extends from anchor to cursor */
    this.selectionAnchor = null;

    // IME state
    /** @type {boolean} */
    this.composing = false;
    /** @type {string} */
    this.compositionText = '';
  }

  /**
   * Enter editing mode for a text element.
   * @param {number} id — text element ID in the engine
   */
  edit(id) {
    this.textId = id;
    this.state = 'editing';
    this.cursorPosition = 0;
    this.selectionAnchor = null;
    this.composing = false;
    this.compositionText = '';
  }

  /**
   * Select a text element (shows bbox, no cursor).
   * @param {number} id
   */
  select(id) {
    this.textId = id;
    this.state = 'selected';
    this.cursorPosition = 0;
    this.selectionAnchor = null;
  }

  /**
   * Deselect / exit any editing mode.
   */
  deselect() {
    this.textId = null;
    this.state = 'idle';
    this.cursorPosition = 0;
    this.selectionAnchor = null;
    this.composing = false;
  }

  /**
   * Commit a content change to the engine and trigger re-measure.
   * @param {string} newContent
   */
  commit(newContent) {
    if (this.textId === null) return;

    this.graph.set_text_content(this.textId, newContent);

    // Trigger immediate re-measure so the next render frame has fresh metrics
    const style = this.graph.get_text(this.textId)?.style;
    if (style) {
      const metrics = this.fontProvider.measure(newContent, style);
      this.graph.set_text_metrics(this.textId, metrics);
    }
  }

  /**
   * @returns {string} current text content from the engine
   */
  getContent() {
    if (this.textId === null) return '';
    const text = this.graph.get_text(this.textId);
    return text ? text.content : '';
  }

  /**
   * Handle keyboard input. Call from the canvas's keydown handler when state === 'editing'.
   * @param {KeyboardEvent} event
   */
  onKeyDown(event) {
    if (this.state !== 'editing' || this.textId === null) return;

    const content = this.getContent();
    const chars = [...content]; // Unicode-aware splitting
    let pos = this.cursorPosition;
    let anchor = this.selectionAnchor;

    const hasSelection = anchor !== null && anchor !== pos;
    const selStart = hasSelection ? Math.min(pos, anchor) : pos;
    const selEnd = hasSelection ? Math.max(pos, anchor) : pos;

    // Helper: replace selection or insert at cursor
    const insert = (text) => {
      const before = chars.slice(0, selStart).join('');
      const after = chars.slice(selEnd).join('');
      this.commit(before + text + after);
      const newPos = selStart + [...text].length;
      this.cursorPosition = newPos;
      this.selectionAnchor = null;
    };

    // Helper: delete selection or character adjacent to cursor
    const deleteAt = (offset, dir) => {
      if (hasSelection) {
        insert('');
        return;
      }
      const idx = pos + offset;
      if (idx < 0 || idx > chars.length) return;
      const before = chars.slice(0, idx).join('');
      const after = chars.slice(idx + 1).join('');
      this.commit(before + after);
      this.cursorPosition = Math.max(0, pos + dir);
      this.selectionAnchor = null;
    };

    const metaOrCtrl = event.metaKey || event.ctrlKey;

    switch (event.key) {
      case 'ArrowLeft':
        event.preventDefault();
        if (metaOrCtrl) {
          // Word left
          let p = Math.max(0, selStart - 1);
          while (p > 0 && chars[p] !== ' ') p--;
          pos = p;
        } else {
          pos = Math.max(0, selStart - 1);
        }
        if (event.shiftKey) {
          anchor = anchor ?? this.cursorPosition;
        } else {
          anchor = null;
        }
        this.cursorPosition = pos;
        this.selectionAnchor = anchor;
        break;

      case 'ArrowRight':
        event.preventDefault();
        if (metaOrCtrl) {
          let p = selEnd + 1;
          while (p < chars.length && chars[p] !== ' ') p++;
          pos = p;
        } else {
          pos = Math.min(chars.length, selEnd + 1);
        }
        if (event.shiftKey) {
          anchor = anchor ?? this.cursorPosition;
        } else {
          anchor = null;
        }
        this.cursorPosition = pos;
        this.selectionAnchor = anchor;
        break;

      case 'Backspace':
        event.preventDefault();
        deleteAt(-1, -1);
        break;

      case 'Delete':
        event.preventDefault();
        deleteAt(0, 0);
        break;

      case 'Enter':
        event.preventDefault();
        insert('\n');
        break;

      case 'Escape':
        event.preventDefault();
        this.deselect();
        break;

      case 'a':
        if (metaOrCtrl) {
          event.preventDefault();
          this.cursorPosition = 0;
          this.selectionAnchor = chars.length;
        }
        break;

      case 'c':
      case 'x':
        if (metaOrCtrl && hasSelection) {
          event.preventDefault();
          const selected = chars.slice(selStart, selEnd).join('');
          navigator.clipboard.writeText(selected).catch(() => {});
          if (event.key === 'x') {
            insert('');
          }
        }
        break;

      case 'v':
        if (metaOrCtrl) {
          event.preventDefault();
          navigator.clipboard.readText().then((text) => {
            if (this.state === 'editing' && this.textId !== null) {
              insert(text);
            }
          }).catch(() => {});
        }
        break;

      default:
        // Printable characters (skip during IME composition)
        if (!this.composing && event.key.length === 1 && !metaOrCtrl) {
          event.preventDefault();
          insert(event.key);
        }
        break;
    }
  }

  /**
   * Handle mouse click on text canvas — position the cursor.
   * @param {number} x — world coordinate
   * @param {number} y — world coordinate
   */
  onMouseDown(x, y) {
    if (this.textId === null || this.state !== 'editing') return;

    const hit = this.graph.get_text_hit(this.textId, x, y);
    if (hit) {
      this.cursorPosition = hit[0];
      this.selectionAnchor = null;
    }
  }

  /**
   * Handle mouse drag to extend selection.
   * @param {number} x
   * @param {number} y
   */
  onMouseDrag(x, y) {
    if (this.textId === null || this.state !== 'editing') return;

    const hit = this.graph.get_text_hit(this.textId, x, y);
    if (hit) {
      if (this.selectionAnchor === null) {
        this.selectionAnchor = this.cursorPosition;
      }
      this.cursorPosition = hit[0];
    }
  }

  // --- IME handlers ---

  onCompositionStart() {
    this.composing = true;
    this.compositionText = '';
  }

  onCompositionUpdate(text) {
    this.compositionText = text;
  }

  onCompositionEnd(text) {
    this.compositionText = '';
    this.composing = false;
    if (text) {
      const content = this.getContent();
      const chars = [...content];
      const hasSelection = this.selectionAnchor !== null && this.selectionAnchor !== this.cursorPosition;
      const selStart = hasSelection ? Math.min(this.cursorPosition, this.selectionAnchor) : this.cursorPosition;
      const selEnd = hasSelection ? Math.max(this.cursorPosition, this.selectionAnchor) : this.cursorPosition;

      const before = chars.slice(0, selStart).join('');
      const after = chars.slice(selEnd).join('');
      this.commit(before + text + after);
      this.cursorPosition = selStart + [...text].length;
      this.selectionAnchor = null;
    }
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add web/text-editor.js
git commit -m "feat: add TextEditor with keyboard, cursor, selection, IME handling"
```

---

### Task 7: Integrate text rendering, cursor, and selection into the demo canvas

**Files:**
- Modify: `web/index.html` — add text state management, render text + cursor + selection in loop, wire keyboard, mouse, IME events

- [ ] **Step 1: Add imports and initialization**

In `web/index.html`, in the `<script type="module">` block, after the existing imports, add:

```javascript
      import { FontManager, FontProvider } from './font-provider.js';
      import { TextEditor } from './text-editor.js';

      // ... (after g = new Graph()) ...

      const fontManager = new FontManager();
      const fontProvider = new FontProvider(fontManager);
      const textEditor = new TextEditor(g, fontProvider);

      // Load a default system font as fallback
      (async () => {
        try {
          // Load Inter from Google Fonts as a built-in option
          const resp = await fetch('https://cdn.jsdelivr.net/npm/@fontsource/inter@5.0.18/files/inter-latin-400-normal.woff');
          const buf = await resp.arrayBuffer();
          await fontManager.load('Inter', buf, 400, 'normal');
          fontManager.setFallbackChain(['Inter', 'sans-serif']);
          console.log('contour: default font loaded');
        } catch (e) {
          console.warn('contour: could not load default font, using browser fallback', e);
        }
      })();
```

- [ ] **Step 2: Add text state fields to the state object**

In the `state` object (around line ~201), add:

```javascript
        // Text state (from existing text tool fields, add new fields):
        textEditorState: 'idle', // 'idle' | 'selected' | 'editing'
        activeTextId: null,
```

Remove the old `textMode` and `textPending` fields and merge them into the new fields:
```javascript
        // Text tool state
        textMode: false,        // keep — toggled by T key
        textPending: null,      // keep — position for new text: [x, y]
```

- [ ] **Step 3: Add text rendering to the render loop**

In the `render()` function, find the existing text rendering section (around line ~820) and replace it with:

```javascript
        // Text elements rendering (live font rendering with editing states)
        try {
          if (typeof g.get_text_ids === 'function') {
            const textIds = g.get_text_ids();
            if (textIds && textIds.length > 0) {
              for (let i = 0; i < textIds.length; i++) {
                const tid = textIds[i];
                const tdata = g.get_text(tid);
                if (!tdata) continue;

                const isEditing = textEditor.state === 'editing' && textEditor.textId === tid;
                const isSelected = textEditor.state === 'selected' && textEditor.textId === tid;
                const tx = tdata.position ? tdata.position.x : 0;
                const ty = tdata.position ? tdata.position.y : 0;
                const style = tdata.style || {};
                const fontSize = style.font_size || 16;
                const fontWeight = style.font_weight || 400;
                const fontFamily = style.font_family || 'sans-serif';
                const fontStyleStr = ['normal', 'italic', 'oblique'][style.font_style] || 'normal';

                // Ensure metrics are fresh
                if (isEditing) {
                  const measureResult = g.measure_text(tid);
                  if (measureResult && measureResult.needs_measure) {
                    const metrics = fontProvider.measure(measureResult.content, measureResult.style);
                    g.set_text_metrics(tid, metrics);
                  }
                }

                ctx.save();
                ctx.translate(tx, ty);
                if (tdata.rotation) ctx.rotate(tdata.rotation);

                const alignVal = typeof tdata.align === 'number' ? tdata.align : 0;
                ctx.textAlign = alignVal === 1 ? 'center' : alignVal === 2 ? 'right' : 'left';
                ctx.textBaseline = 'alphabetic';

                // Font style string
                ctx.font = `${fontStyleStr} ${fontWeight} ${fontSize}px "${fontFamily}", sans-serif`;

                // Fill color
                if (style.fill_color) {
                  const fc = style.fill_color;
                  ctx.fillStyle = `rgba(${fc.r || 0},${fc.g || 0},${fc.b || 0},${(fc.a || 255)/255})`;
                } else {
                  ctx.fillStyle = '#000';
                }

                // Render text content
                if (tdata.text_type && tdata.text_type.type === 'on_path') {
                  // Text on path: draw each character individually
                  const edgeIds = new Uint32Array(tdata.text_type.edge_ids || []);
                  const totalLen = edgeIds.length > 0 ? g.path_length(edgeIds) : 0;
                  if (totalLen > 0) {
                    let currentDist = (tdata.text_type.start_offset || 0) * totalLen;
                    const content = tdata.content || '';

                    // Get char widths from metrics
                    let charWidths = [];
                    const tmp = g.measure_text(tid);
                    if (tmp && !tmp.needs_measure && tmp.char_widths) {
                      charWidths = tmp.char_widths;
                    }

                    for (let ci = 0; ci < content.length; ci++) {
                      const pt = g.sample_path_point(edgeIds, currentDist);
                      if (!pt) break;

                      ctx.save();
                      ctx.translate(pt.x, pt.y);
                      ctx.rotate(pt.angle);
                      ctx.fillText(content[ci], 0, 0);
                      ctx.restore();

                      currentDist += charWidths[ci] || fontSize * 0.6;
                    }
                  }
                } else {
                  // Label or TextBox: standard fillText
                  ctx.fillText(tdata.content || '', 0, 0);
                }

                // Selection highlight (if editing with selection)
                if (isEditing) {
                  const anchor = textEditor.selectionAnchor;
                  const cursor = textEditor.cursorPosition;
                  if (anchor !== null && anchor !== cursor) {
                    const start = Math.min(anchor, cursor);
                    const end = Math.max(anchor, cursor);
                    const quads = g.get_text_selection_bounds(tid, start, end);
                    if (quads) {
                      ctx.fillStyle = 'rgba(30, 111, 235, 0.3)';
                      for (const q of quads) {
                        ctx.fillRect(q[0] - tx, q[1] - ty, q[2], q[3]);
                      }
                    }
                  }

                  // Cursor blink (if not composing)
                  if (!textEditor.composing) {
                    const positions = g.get_text_char_positions(tid);
                    if (positions) {
                      const pos = positions.find(p => p.char_index === textEditor.cursorPosition);
                      if (pos) {
                        if (Math.floor(Date.now() / 500) % 2 === 0) {
                          ctx.fillStyle = '#000';
                          const lineH = fontSize * (style.line_height || 1.2);
                          ctx.fillRect(pos.x, pos.y - fontSize, 1, lineH);
                        }
                      }
                    }
                  }

                  // IME composition underline
                  if (textEditor.composing && textEditor.compositionText) {
                    const positions = g.get_text_char_positions(tid);
                    if (positions) {
                      const pos = positions.find(p => p.char_index === textEditor.cursorPosition);
                      if (pos) {
                        const compWidth = textEditor.compositionText.length * fontSize * 0.6;
                        ctx.strokeStyle = '#000';
                        ctx.lineWidth = 1;
                        ctx.beginPath();
                        ctx.moveTo(pos.x, pos.y + 2);
                        ctx.lineTo(pos.x + compWidth, pos.y + 2);
                        ctx.stroke();
                      }
                    }
                  }
                }

                // Selection bbox outline
                if (isSelected) {
                  const positions = g.get_text_char_positions(tid);
                  if (positions && positions.length > 0) {
                    let minX = Infinity, maxX = -Infinity;
                    let minY = Infinity, maxY = -Infinity;
                    for (const p of positions) {
                      minX = Math.min(minX, p.x);
                      maxX = Math.max(maxX, p.x + p.w);
                      minY = Math.min(minY, p.y - fontSize);
                      maxY = Math.max(maxY, p.y + fontSize * 0.3);
                    }
                    ctx.strokeStyle = '#d73a49';
                    ctx.lineWidth = 1;
                    ctx.setLineDash([3, 3]);
                    ctx.strokeRect(minX - 2, minY - 2, maxX - minX + 4, maxY - minY + 4);
                    ctx.setLineDash([]);
                  }
                }

                ctx.restore();
              }
            }
          }
        } catch (e) { console.warn('Text render error:', e); }
```

- [ ] **Step 4: Wire keyboard events for text editing**

In the demo's keyboard handler section, add text editing key handling before the existing tool shortcuts:

```javascript
      // Text editing keyboard events
      window.addEventListener('keydown', (e) => {
        if (textEditor.state === 'editing') {
          textEditor.onKeyDown(e);
          if (['ArrowLeft','ArrowRight','Backspace','Delete','Enter','Escape',
               'ArrowUp','ArrowDown'].includes(e.key) ||
              (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) ||
              ((e.ctrlKey || e.metaKey) && ['a','c','x','v'].includes(e.key.toLowerCase()))) {
            render();
            return;
          }
        }
      });
```

- [ ] **Step 5: Wire IME composition events**

Add after the keyboard handler:

```javascript
      const canvasEl = document.getElementById('canvas');
      canvasEl.addEventListener('compositionstart', () => {
        if (textEditor.state === 'editing') textEditor.onCompositionStart();
      });
      canvasEl.addEventListener('compositionupdate', (e) => {
        if (textEditor.state === 'editing') {
          textEditor.onCompositionUpdate(e.data);
          render();
        }
      });
      canvasEl.addEventListener('compositionend', (e) => {
        if (textEditor.state === 'editing') {
          textEditor.onCompositionEnd(e.data);
          render();
        }
      });
```

- [ ] **Step 6: Wire text tool interactions (create, click to select, double-click to edit)**

In the existing pointer event handlers, update text-related logic:

- **Create text on click** (when `state.textMode` is true): creates text via `g.add_text()`, then calls `textEditor.edit(id)` to enter editing. The existing `state.textPending` code handles this.

- **Click on existing text**: after picking, if pick is null and `state.textMode` is false, check if any text is under the cursor:
```javascript
      // In onPointerDown, after the pick check:
      if (!pick && !state.textMode) {
        // Check text hit
        const textIds = g.get_text_ids();
        let hitText = false;
        for (const tid of textIds) {
          const thit = g.get_text_hit(tid, point.x, point.y);
          if (thit) {
            if (textEditor.textId === tid && textEditor.state === 'selected') {
              // Double-click detected via existing dblclick handler → editing
              textEditor.edit(tid);
            } else {
              textEditor.select(tid);
            }
            hitText = true;
            break;
          }
        }
        if (hitText) {
          onSelectNode(null);
          render();
          return;
        }
      }
```

- **Double-click**: modify existing `onDoubleClick` to also handle text:
```javascript
      const onDoubleClick = (event) => {
        const canvas = canvasRef ? canvasRef.current : document.getElementById('canvas');
        const rect = canvas.getBoundingClientRect();
        const point = {
          x: event.clientX - rect.left,
          y: event.clientY - rect.top,
        };

        // Check if double-clicking on text
        const textIds = g.get_text_ids();
        for (const tid of textIds) {
          const thit = g.get_text_hit(tid, point.x, point.y);
          if (thit) {
            textEditor.edit(tid);
            textEditor.onMouseDown(point.x, point.y);
            updateModeLabel();
            render();
            return;
          }
        }

        // Default: add node at position
        const id = g.add_node(point.x, point.y, onSelectNode);
        onSelectNode(id);
        render();
      };
```

- [ ] **Step 7: Update updateModeLabel to reflect text editing state**

In `updateModeLabel()`, add text editing state:

```javascript
        if (textEditor.state === 'editing') mode = 'Editing Text';
        else if (textEditor.state === 'selected') mode = 'Text Selected';
        else if (state.textMode) mode = 'Text';
```

- [ ] **Step 8: Handle click-away to deselect text**

In the pointer handler, at the top of `onPointerDown`:

```javascript
      // Click-away: deselect text if clicking elsewhere
      if (textEditor.state !== 'idle') {
        const textIds = g.get_text_ids();
        let hitText = false;
        for (const tid of textIds) {
          const thit = g.get_text_hit(tid, point.x, point.y);
          if (thit) { hitText = true; break; }
        }
        if (!hitText && !pick) {
          textEditor.deselect();
          updateModeLabel();
          render();
        }
      }
```

- [ ] **Step 9: Commit**

```bash
git add web/index.html
git commit -m "feat: integrate live text rendering, cursor, selection, IME into demo canvas"
```

---

### Task 8: End-to-end verification and final commit

**Files:**
- None new — verify everything works

- [ ] **Step 1: Run all Rust tests**

```bash
cargo test -p contour 2>&1
```

Expected: all tests pass (56+ existing + new text_metrics tests).

- [ ] **Step 2: Build WASM for the web demo**

```bash
cd contour-wasm && wasm-pack build --target web --out-dir ../web/pkg 2>&1
```

Expected: builds without errors.

- [ ] **Step 3: Run WASM integration tests**

```bash
cd contour-wasm && wasm-pack test --node 2>&1
```

Expected: all WASM tests pass.

- [ ] **Step 4: Manually verify the demo**

```bash
python3 -m http.server 8000
# Open http://localhost:8000/web/index.html
```

Verify:
1. Press T, click canvas → text "Text" appears, blinking cursor visible
2. Type characters → text updates live
3. Arrow keys move cursor
4. Shift+arrow selects text (blue highlight)
5. Click away → exits editing
6. Click text → shows selection bbox
7. Double-click text → enters editing again
8. Press C → converts text to cubic edge (existing feature, still works)

- [ ] **Step 5: Commit final changes**

```bash
git add -A && git status
git commit -m "chore: final verification — text rendering integration complete"
```

---