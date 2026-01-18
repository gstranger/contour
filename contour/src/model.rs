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
