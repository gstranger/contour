use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Fill rule for determining inside/outside of shapes
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum FillRule {
    /// Non-zero winding rule (standard for most vector graphics)
    #[default]
    NonZero = 0,
    /// Even-odd rule (alternating fills)
    EvenOdd = 1,
}

/// A shape is an ordered collection of edges forming a closed or open path
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub id: u32,
    /// Edge IDs forming this shape, in order
    pub edges: Vec<u32>,
    /// Whether the shape forms a closed loop
    pub closed: bool,
    /// Fill rule for determining inside/outside
    pub fill_rule: FillRule,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FillState {
    pub filled: bool,
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Node {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum HandleMode {
    Free = 0,
    Mirrored = 1,
    Aligned = 2,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EdgeKind {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub kind: EdgeKind,
    pub stroke: Option<Color>,
    pub stroke_width: f32,
}

// --- Layer/Group System ---

/// Layer identifier type
pub type LayerId = u32;

/// A group is a container for edges within a layer hierarchy
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: LayerId,
    pub name: String,
    /// Parent group ID, None if this is a layer's root group
    pub parent: Option<LayerId>,
    /// Child group IDs
    pub children: Vec<LayerId>,
    /// Edge IDs directly in this group
    pub edges: Vec<u32>,
    pub visible: bool,
    pub locked: bool,
    /// Opacity from 0.0 to 1.0
    pub opacity: f32,
}

/// A layer is a top-level organizational container with a root group
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    /// Z-index for ordering (higher = on top)
    pub z_index: i32,
    pub visible: bool,
    pub locked: bool,
    /// Opacity from 0.0 to 1.0
    pub opacity: f32,
    /// The root group for this layer
    pub root_group: LayerId,
}

// --- Gradient System ---

/// Gradient identifier type
pub type GradientId = u32;

/// A color stop within a gradient
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ColorStop {
    /// Position in gradient (0.0 to 1.0)
    pub offset: f32,
    pub color: Color,
}

/// Gradient coordinate system
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum GradientUnits {
    /// Coordinates relative to object bounding box (0-1)
    #[default]
    ObjectBoundingBox = 0,
    /// Absolute coordinates in user space
    UserSpaceOnUse = 1,
}

/// Spread method when gradient doesn't cover entire object
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SpreadMethod {
    /// Extend edge colors
    #[default]
    Pad = 0,
    /// Mirror gradient
    Reflect = 1,
    /// Tile gradient
    Repeat = 2,
}

/// Linear gradient definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinearGradient {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stops: Vec<ColorStop>,
    pub units: GradientUnits,
    pub spread: SpreadMethod,
}

/// Radial gradient definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadialGradient {
    /// Center x
    pub cx: f32,
    /// Center y
    pub cy: f32,
    /// Radius
    pub r: f32,
    /// Focal point x (optional, defaults to cx)
    pub fx: f32,
    /// Focal point y (optional, defaults to cy)
    pub fy: f32,
    pub stops: Vec<ColorStop>,
    pub units: GradientUnits,
    pub spread: SpreadMethod,
}

/// Unified gradient type
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Gradient {
    Linear(LinearGradient),
    Radial(RadialGradient),
}

/// Paint type - unified fill/stroke specification
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Paint {
    /// No paint (transparent)
    None,
    /// Solid color
    Solid { color: Color },
    /// Reference to a gradient by ID
    Gradient { id: GradientId },
}

// --- Text System ---

/// Text identifier type
pub type TextId = u32;

/// Font weight (100-900)
pub type FontWeight = u16;

/// Font style
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum FontStyle {
    #[default]
    Normal = 0,
    Italic = 1,
    Oblique = 2,
}

/// Text alignment
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left = 0,
    Center = 1,
    Right = 2,
}

/// Vertical text alignment (for text boxes)
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerticalAlign {
    #[default]
    Top = 0,
    Middle = 1,
    Bottom = 2,
}

/// Text overflow behavior (for text boxes)
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TextOverflow {
    /// Clip text at boundaries
    #[default]
    Clip = 0,
    /// Show ellipsis for overflow
    Ellipsis = 1,
    /// Allow text to overflow boundaries
    Visible = 2,
}

/// Text styling properties
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextStyle {
    /// Font family name (e.g., "Arial", "Helvetica")
    pub font_family: String,
    /// Font size in points
    pub font_size: f32,
    /// Font weight (100-900, normal=400, bold=700)
    pub font_weight: FontWeight,
    /// Font style (normal, italic, oblique)
    pub font_style: FontStyle,
    /// Fill color for text
    pub fill_color: Option<Color>,
    /// Stroke color for text outline
    pub stroke_color: Option<Color>,
    /// Stroke width for text outline
    pub stroke_width: f32,
    /// Letter spacing in em units
    pub letter_spacing: f32,
    /// Line height multiplier (1.0 = normal)
    pub line_height: f32,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "sans-serif".to_string(),
            font_size: 16.0,
            font_weight: 400,
            font_style: FontStyle::Normal,
            fill_color: Some(Color { r: 0, g: 0, b: 0, a: 255 }),
            stroke_color: None,
            stroke_width: 0.0,
            letter_spacing: 0.0,
            line_height: 1.2,
        }
    }
}

/// Type of text element
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TextType {
    /// Simple positioned text label
    Label,
    /// Text box with wrapping
    Box {
        /// Box width in pixels
        width: f32,
        /// Box height in pixels
        height: f32,
        /// Vertical alignment within box
        vertical_align: VerticalAlign,
        /// Overflow behavior
        overflow: TextOverflow,
    },
    /// Text that flows along a path
    OnPath {
        /// Edge IDs forming the path
        edge_ids: Vec<u32>,
        /// Starting offset along path (0.0 to 1.0)
        start_offset: f32,
    },
}

impl Default for TextType {
    fn default() -> Self {
        TextType::Label
    }
}

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
}

impl TextElement {
    /// Create a new simple text label
    pub fn new_label(id: TextId, content: String, x: f32, y: f32) -> Self {
        Self {
            id,
            content,
            position: Vec2 { x, y },
            rotation: 0.0,
            style: TextStyle::default(),
            align: TextAlign::Left,
            text_type: TextType::Label,
        }
    }

    /// Create a new text box
    pub fn new_box(id: TextId, content: String, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            id,
            content,
            position: Vec2 { x, y },
            rotation: 0.0,
            style: TextStyle::default(),
            align: TextAlign::Left,
            text_type: TextType::Box {
                width,
                height,
                vertical_align: VerticalAlign::Top,
                overflow: TextOverflow::Clip,
            },
        }
    }

    /// Create text on a path
    pub fn new_on_path(id: TextId, content: String, edge_ids: Vec<u32>) -> Self {
        Self {
            id,
            content,
            position: Vec2 { x: 0.0, y: 0.0 }, // Position determined by path
            rotation: 0.0,
            style: TextStyle::default(),
            align: TextAlign::Left,
            text_type: TextType::OnPath {
                edge_ids,
                start_offset: 0.0,
            },
        }
    }
}

/// Glyph outline for text-to-outlines conversion
/// (Provided by JavaScript via font parsing library)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlyphOutline {
    /// Character this glyph represents
    pub char: char,
    /// Advance width (spacing to next character)
    pub advance_width: f32,
    /// Glyph paths (contours)
    pub paths: Vec<GlyphPath>,
}

/// A single contour of a glyph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlyphPath {
    /// Path commands forming the contour
    pub commands: Vec<PathCommand>,
}

/// SVG-like path command for glyph outlines
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PathCommand {
    /// Move to position
    MoveTo(f32, f32),
    /// Line to position
    LineTo(f32, f32),
    /// Quadratic bezier (control point, end point)
    QuadTo(f32, f32, f32, f32),
    /// Cubic bezier (control1, control2, end point)
    CubicTo(f32, f32, f32, f32, f32, f32),
    /// Close the path
    Close,
}

/// Line of laid out text (for text boxes)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutLine {
    /// The text content of this line
    pub text: String,
    /// Y offset from text box top
    pub y_offset: f32,
    /// X offset (for alignment)
    pub x_offset: f32,
    /// Width of this line in pixels
    pub width: f32,
}
