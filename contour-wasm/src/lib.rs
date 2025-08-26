use wasm_bindgen::prelude::*;
mod api;
mod interop;

#[wasm_bindgen]
pub struct Graph { pub(crate) inner: contour::Graph }

impl Graph {
    pub fn rs_new() -> Graph { Graph { inner: contour::Graph::new() } }
    pub fn rs_geom_version(&self) -> u64 { self.inner.geom_version() }
}

