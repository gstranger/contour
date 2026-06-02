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
    pub fn rs_measure_text(&self, id: u32) -> Option<contour::model::TextMeasureResult> {
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
}
