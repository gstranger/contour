use wasm_bindgen::prelude::*;
mod api;
mod error;
mod interop;

#[wasm_bindgen]
pub struct Graph {
    pub(crate) inner: contour::Graph,
}

impl Graph {
    pub fn rs_new() -> Graph {
        Graph {
            inner: contour::Graph::new(),
        }
    }
    pub fn rs_geom_version(&self) -> u64 {
        self.inner.geom_version()
    }
    pub fn rs_begin_undo_group(&mut self, label: String) {
        self.inner.begin_undo_group(label);
    }
    pub fn rs_end_undo_group(&mut self) -> bool {
        self.inner.end_undo_group()
    }
    pub fn rs_undo(&mut self) -> Option<(String, usize)> {
        self.inner.undo()
    }
    pub fn rs_redo(&mut self) -> Option<(String, usize)> {
        self.inner.redo()
    }
    pub fn rs_undo_depth(&self) -> (usize, usize) {
        self.inner.undo_depth()
    }
    pub fn rs_undo_clear(&mut self) {
        self.inner.undo_clear();
    }
}
