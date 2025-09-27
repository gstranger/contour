use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
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
