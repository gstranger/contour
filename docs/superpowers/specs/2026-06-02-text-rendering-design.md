# Text Rendering — Design Spec

**Date:** 2026-06-02  
**Status:** Approved  
**Scope:** Phase 2 (Near-Term) — integrate live font rendering, inline text editing, and text-on-path into the contour vector network engine.

---

## 1. Problem

The engine has a complete text data model (`TextElement`, `TextStyle`, `TextType`), a layout engine (word wrap, alignment, truncation), and a text→outlines pipeline (`GlyphOutline` → graph edges) — but no actual font rendering. Text cannot be seen or edited without an external font library feeding it glyph metrics and outline data.

The goal is to add live text rendering and inline editing while preserving the engine's role as a pure state manager (not a renderer).

---

## 2. Architecture

### 2.1 Principle

The engine manages text **state and layout**. JS manages text **rendering and input**. The boundary is clean — the engine never calls into JS; JS polls the engine for measurement needs, same pattern as `get_dirty()`.

### 2.2 Data Flow

```
┌─ JS side ─────────────────────────────────────────────────────┐
│                                                                │
│  FontManager (loads .ttf/.otf, caches, fallback chain)        │
│       │                                                        │
│  FontProvider (measure text, generate glyph outlines)          │
│       │                                                        │
│       ├── measures → TextMetrics                               │
│       │                                                        │
│  TextEditor (keyboard, cursor, selection, IME, clipboard)     │
│       │                                                        │
│  Canvas Renderer                                               │
│    - ctx.fillText() for live text                              │
│    - Cursor blink overlay                                      │
│    - Selection highlight overlay                               │
│    - Text-on-path via per-glyph transform                      │
│       │                                                        │
├───────┼──────────────────────────────────────── WASM boundary ─┤
│       ▼                                                        │
│  Engine (Rust)                                                 │
│    TextElement ← set_text_content                              │
│    + metrics cache (per-element, versioned)                   │
│    text_layout (word wrap, alignment)                          │
│    text_outline (destructive convert → graph edges)            │
│    char_positions (hit test, cursor placement)                 │
│    path_sampling (for text-on-path positioning)                │
└────────────────────────────────────────────────────────────────┘
```

### 2.3 Why Hybrid (FontProvider in JS, Engine Holds Cache)

The engine already receives `char_widths` and `GlyphOutline[]` from external callers — this was the original design intent. Keeping fonts in JS:

- Preserves browser-native text rendering (`fillText()` produces superior results at UI sizes via hinting and subpixel AA)
- Keeps WASM small (no ttf-parser, harfbuzz, or shaping tables compiled in)
- Allows swapping the font stack (opentype.js → harfbuzzjs) without touching the engine
- Follows the existing dirty/poll pattern (`get_dirty()`) rather than introducing callback complexity

Metrics caching lives in the engine (per `TextElement`, versioned) so the JS renderer can read cached layout without crossing the boundary every frame.

---

## 3. Engine Additions (Rust → WASM)

### 3.1 Metrics Cache

```
TextElement gains:
  metrics_ver: u64          ← bumped when content or style changes
  cached_metrics: Option<TextMetrics>   ← cleared on version mismatch
  cached_layout: Option<TextBoxLayout>   ← recomputed from metrics
```

### 3.2 New WASM Methods

| Method | Returns | Purpose |
|--------|---------|---------|
| `measure_text(id: u32)` | `null \| { cached: TextMetrics } \| { needs_measure: NeedsMeasure }` | Poll if this text needs JS-side measurement. Returns cached metrics if fresh, or a measurement request if content/style changed. |
| `set_text_metrics(id: u32, metrics: TextMetrics)` | `bool` | Store JS-measured metrics in the engine cache. Returns false if id is invalid. |
| `get_text_char_positions(id: u32)` | `[{x: f32, y: f32, w: f32, charIndex: u32}] \| null` | Pixel positions of every character for cursor placement and hit testing. |
| `get_text_hit(id: u32, x: f32, y: f32)` | `{charIndex: u32, lineIndex: u32} \| null` | Which character is at world-coordinate (x, y). Uses cached layout. |
| `get_text_selection_bounds(id: u32, start: u32, end: u32)` | `[{x: f32, y: f32, w: f32, h: f32}] \| null` | Rectangles for rendering the selection highlight over a character range. |
| `sample_path_point(edge_ids: &Uint32Array, distance: f32)` | `{x: f32, y: f32, angle: f32} \| null` | Position and tangent angle at a given distance along a chain of edges. For text-on-path per-glyph placement. |

### 3.3 Type Definitions

```
TextMetrics:
  char_widths: Float32Array      // width of each character in pixels
  line_height: f32               // computed line height
  ascent: f32                    // distance from baseline to top
  descent: f32                   // distance from baseline to bottom
  total_width: f32               // full width of the rendered text

NeedsMeasure:
  content: string
  style: TextStyle               // font_family, font_size, font_weight, etc.
```

### 3.4 Existing Methods (no changes)

The following already exist and work with no modification:

- `layout_text_box(content, width, height, style, char_widths, align, valign)` → `TextBoxLayout`
- `get_character_positions(layout, char_widths, letter_spacing, font_size)` → `[(x, y, w)]`
- `text_to_outlines(text_id, glyphs: &[GlyphOutline])` → `TextOutlineResult`
- `point_on_path(edge_ids, distance)` → `(x, y, tangent_angle)` (in `geometry/path_length.rs`)
- `sample_text_positions(edge_ids, distances)` → `[(x, y, angle)]` (in `geometry/path_length.rs`)

---

## 4. JS-Side Components

### 4.1 FontManager

```typescript
class FontManager {
  // Load a font from ArrayBuffer or URL
  async load(family: string, source: ArrayBuffer | string, weight?: number, style?: string): Promise<void>
  
  // Remove a loaded font
  unload(family: string): void
  
  // Configure fallback order
  setFallbackChain(chain: string[]): void
  
  // Internal: resolve a font family+weight+style to an opentype.Font object
  // Walks the fallback chain if exact match fails
  resolveFont(family: string, weight: number, style: string): opentype.Font | null
  
  // Internal: per-(font,size) glyph metrics cache
  // Map<char, {width, xMin, xMax}> — avoids re-parsing glyph tables
  glyphMetricsCache: Map<string, Map<string, GlyphMetrics>>
}
```

Uses `opentype.js` (MIT, ~200KB) for font parsing and glyph measurement. The `glyphMetricsCache` accumulates per-character measurements; typical documents use < 200 unique characters, keeping cache small.

### 4.2 FontProvider

```typescript
interface FontProvider {
  measure(content: string, style: TextStyle): TextMetrics
  getOutlines(content: string, style: TextStyle): GlyphOutline[]
}
```

A thin adapter over FontManager. `measure()` resolves the font, iterates characters against the glyph cache, builds the `TextMetrics` struct. `getOutlines()` calls `font.getPath()` per character and converts to `GlyphOutline[]` format.

### 4.3 TextEditor

```typescript
class TextEditor {
  textId: u32
  cursorPosition: u32          // char index
  selectionAnchor: u32 | null  // if set, selection from anchor → cursor
  
  // IME state
  composing: boolean
  compositionRange: [u32, u32] | null
  compositionText: string       // not yet committed to engine
  
  // Keyboard handling
  onKeyDown(event: KeyboardEvent): void
    // arrows: move cursor (with shift: extend selection)
    // backspace/delete: remove chars, commit to engine
    // printable chars: insert at cursor, commit to engine
    // cmd+A: select all
    // cmd+C/X/V: clipboard (read/write navigator.clipboard)
    // Escape: exit editing → IDLE state
  
  // Mouse handling
  onMouseDown(x: f32, y: f32): void
    // hit-test via get_text_hit() → set cursor + clear selection
    // drag: extend selection
  
  // IME handling
  onCompositionStart(): void
  onCompositionUpdate(text: string): void
  onCompositionEnd(text: string): void
  
  // Commit content change to engine + trigger re-measure
  private commit(content: string): void
}
```

### 4.4 Rendering Pipeline (per-frame, in existing render loop)

```
For each TextElement in engine:

  // --- Active editing state ---
  if textId == activeEditor.textId:

    if activeEditor.state == EDITING:
      1. Poll: result = g.measure_text(textId)
         if result.needs_measure:
           metrics = fontProvider.measure(result.content, result.style)
           g.set_text_metrics(textId, metrics)
           result = g.measure_text(textId)  // now returns cached
      
      2. positions = g.get_text_char_positions(textId)
      
      3. Draw text: ctx.fillText(content, x, y)  // browser renders glyphs
      
      4. If selection active:
         quads = g.get_text_selection_bounds(textId, anchor, cursor)
         for each quad: ctx.fillRect with semi-transparent blue
      
      5. Draw cursor (if not composing):
         cursorPos = positions[activeEditor.cursorPosition]
         if (Date.now() % 1000 < 500):  // blink
           ctx.fillRect(cursorPos.x, cursorPos.y - ascent, 1, lineHeight)
      
      6. If IME composing:
         Draw compositionText at cursor position with browser's default underline

    if activeEditor.state == SELECTED:
      Draw text normally (ctx.fillText)
      Draw bounding box + 8 transform handles (same code as node/edge selection)
  
  // --- Idle state ---
  else:
    Draw text normally: ctx.fillText(content, x, y)
    Apply rotation via ctx.rotate(text.rotation) before drawing
```

### 4.5 Text States

Three mutually exclusive UI states per text element:

| State | Entry | Shows | Cursor |
|-------|-------|-------|--------|
| IDLE | Default / click away | Rendered text only | default |
| SELECTED | Single-click on text block | Text + bounding box + 8 transform handles | move |
| EDITING | Double-click on text block | Text + blinking cursor + selection highlight | text |

### 4.6 Hit Testing Order (top → bottom)

1. Cursor/selection grabbers (if EDITING state)
2. Transform handles (if SELECTED state)  
3. Text characters (`get_text_hit()`)
4. Edge handles (if edge selected)
5. Edges
6. Nodes

### 4.7 Text on Path Rendering

For `TextType::OnPath { edge_ids, start_offset }`:

```
totalLen = compute total path length from engine
currentDist = start_offset * totalLen

for each char in content:
  point = g.sample_path_point(edge_ids, currentDist)
  if point is null: break
  
  ctx.save()
  ctx.translate(point.x, point.y)
  ctx.rotate(point.angle)
  ctx.fillText(char, 0, 0)  // baseline-aligned at origin after transform
  ctx.restore()
  
  currentDist += char.width  // from FontProvider.measure()
```

---

## 5. Incremental Cache Strategy

```
measure_text(textId):
  if metrics_ver == text.metrics_ver:
    return { cached: layout_from(metrics) }
  else:
    cache miss → return { needs_measure: { content, style } }

set_text_metrics(textId, metrics):
  text.cached_metrics = metrics
  text.metrics_ver = text.metrics_ver  // sync version
  recompute layout
  return true

set_text_content(textId, content):
  text.content = content
  text.metrics_ver += 1    // invalidate cache
  clear cached_metrics
  bump geom_ver
```

Key property: `measure_text()` returns cached results immediately if nothing changed. The JS render loop calls it every frame unconditionally; it's cheap (a u64 compare) when cached.

---

## 6. IME Composition

The engine never sees partial IME strings. During composition:

```
compositionstart  → TextEditor.composing = true
compositionupdate → render inline (browser handles underline decoration)
compositionend    → commit final text to engine via set_text_content()
                     → engine updates content, bumps version
                     → TextEditor.cursorPosition advances
                     → TextEditor.composing = false
```

This avoids partial strings polluting undo history or triggering half-formed glyph measurement.

---

## 7. Missing Font Handling

If `FontManager.resolveFont()` returns null (no font in chain matches):

- Render text content in red (`ctx.fillStyle = "#d73a49"`) with a single underline
- Match Figma's "missing font" behavior
- Show a warning in the HUD: "Missing font: Inter"

---

## 8. Testing Strategy

| Test | Type | Location | Coverage |
|------|------|----------|----------|
| `layout_text_box` with known widths | Unit | Rust | Wrap, align, truncation (exists, extend) |
| `get_text_hit` at known positions | Unit | Rust | Character index accuracy |
| `text_to_outlines` round-trip | Unit | Rust | Text → edges → regions (exists) |
| Measure cache invalidation | Integration | wasm-bindgen-test | Edit content → cache miss → set → cache hit |
| `sample_path_point` accuracy | Unit | Rust | Straight line, cubic curve |
| Font fallback chain | Integration | JS (Cypress) | Request missing font → resolve via chain |
| IME composition (CJK) | Visual | JS (Playwright) | Compose → commit → text matches |
| Cursor + selection inputs | Visual | JS (Playwright) | Arrow keys, shift+arrow, click+drag |
| Text on path visual correctness | Visual | JS (Playwright) | Glyphs follow curve with correct rotation |

---

## 9. Explicitly Out of Scope

- Mixed formatting (bold/italic within a single TextElement) — requires `TextRun[]` model
- Variable font axis controls — opentype.js supports reading, but exposing sliders is future work
- Font subsetting for document portability
- WebGL text rendering — Canvas 2D `fillText()` only initially
- Text styles / design tokens (global presets)
- Right-to-left / bidirectional text — opentype.js does not handle it; would need harfbuzzjs
- Font embedding in SVG/JSON export (text exports as `<text>` element with `font-family` reference)

---

## 10. Dependencies

- **JS:** `opentype.js` (MIT, ~200KB) — font loading, glyph measurement, outline extraction
- **Rust:** No new dependencies. Existing `geometry/path_length.rs` is sufficient for path sampling.
- **WASM binary:** No size increase (no Rust font stack compiled in)

---

## 11. Files Changed

| File | Change |
|------|--------|
| `contour/src/model.rs` | Add `TextMetrics` struct, `metrics_ver` and `cached_*` fields to `TextElement` |
| `contour/src/lib.rs` | Add `measure_text()`, `set_text_metrics()`, `get_text_char_positions()`, `get_text_hit()`, `get_text_selection_bounds()` methods on `Graph` |
| `contour/src/geometry/path_length.rs` | Expose `sample_path_point()` publicly |
| `contour-wasm/src/api.rs` | WASM bindings for all new methods (with `_res` strict variants) |
| `contour-wasm/types.d.ts` | TypeScript declarations for new methods and types |
| `web/index.html` | Add `FontManager`, `FontProvider`, `TextEditor`; integrate text rendering into canvas render loop |
| `web/font-provider.js` (new) | `FontManager` + `FontProvider` implementation using opentype.js |
| `web/text-editor.js` (new) | `TextEditor` class with keyboard/mouse/IME handling |
| `contour-wasm/tests/text_metrics_tests.rs` (new) | WASM integration tests for the measure/cache pipeline |
| `docs/` | This design doc, updated API reference |