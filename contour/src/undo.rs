//! Undo/redo system using hybrid model:
//! - Structural commands for creation/destruction (add/remove node, edge, shape, text)
//! - Targeted snapshots for mutative operations (move, bend, style changes)

use crate::model::{Edge, FillState, Node, Shape, TextElement};
use std::collections::VecDeque;

pub const MAX_UNDO_DEPTH: usize = 256;

/// A stack of undo or redo entries. Evicts oldest when cap exceeded.
pub(crate) struct UndoStack {
    entries: VecDeque<UndoEntry>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_UNDO_DEPTH),
        }
    }

    pub fn push(&mut self, entry: UndoEntry) {
        if self.entries.len() >= MAX_UNDO_DEPTH {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn pop(&mut self) -> Option<UndoEntry> {
        self.entries.pop_back()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Peek at the top entry's label without removing it.
    #[allow(dead_code)]
    pub fn top_label(&self) -> Option<&str> {
        self.entries.back().map(|e| e.label.as_str())
    }
}

/// A recorded undo step.
pub(crate) struct UndoEntry {
    pub label: String,
    pub action: UndoAction,
}

/// The payload of an undo entry.
pub(crate) enum UndoAction {
    /// Structural creation/destruction — carries its own inverse.
    Command(UndoCommand),
    /// State snapshot from a gesture group.
    Snapshot(SnapshotBatch),
}

/// Structural commands for inverting creation/destruction.
#[allow(dead_code)]
pub(crate) enum UndoCommand {
    AddNode {
        id: u32,
        pos: (f32, f32),
    },
    RemoveNode {
        id: u32,
        pos: (f32, f32),
        incident: Vec<(u32, Edge)>,
    },
    AddEdge {
        id: u32,
        a: u32,
        b: u32,
        edge: Edge,
    },
    RemoveEdge {
        id: u32,
        endpoint_a: u32,
        endpoint_b: u32,
        edge: Edge,
    },
    AddShape {
        id: u32,
        shape: Shape,
    },
    RemoveShape {
        id: u32,
        shape: Shape,
    },
    AddText {
        id: u32,
        text: TextElement,
    },
    RemoveText {
        id: u32,
        text: TextElement,
    },
}

/// Holds the "before" state of all elements touched during a gesture group.
pub(crate) struct SnapshotBatch {
    pub nodes: Vec<(u32, Option<Node>)>,
    pub edges: Vec<(u32, Option<Edge>)>,
    pub fills: Vec<(u32, Option<FillState>)>,
    pub shapes: Vec<(u32, Option<Shape>)>,
    pub texts: Vec<(u32, Option<TextElement>)>,
}

impl SnapshotBatch {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            fills: Vec::new(),
            shapes: Vec::new(),
            texts: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.edges.is_empty()
            && self.fills.is_empty()
            && self.shapes.is_empty()
            && self.texts.is_empty()
    }
}
