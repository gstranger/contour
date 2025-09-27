#[cfg(feature = "bench_regions")]
use contour::Graph;
#[cfg(feature = "bench_regions")]
use std::time::Instant;

#[cfg(not(feature = "bench_regions"))]
fn main() {
    panic!("regions_bench requires --features bench_regions");
}

#[cfg(feature = "bench_regions")]
fn build_grid_graph(w: usize, h: usize) -> Graph {
    let mut g = Graph::new();
    let mut nodes = Vec::with_capacity((w + 1) * (h + 1));
    for j in 0..=h {
        for i in 0..=w {
            nodes.push(g.add_node(i as f32 * 6.0, j as f32 * 6.0));
        }
    }
    let ix = |i: usize, j: usize| -> u32 { nodes[j * (w + 1) + i] };
    for j in 0..=h {
        for i in 0..w {
            g.add_edge(ix(i, j), ix(i + 1, j));
        }
    }
    for i in 0..=w {
        for j in 0..h {
            g.add_edge(ix(i, j), ix(i, j + 1));
        }
    }
    g
}

#[cfg(feature = "bench_regions")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut w = 70usize; // ~ (w+1)*h + (h+1)*w edges; choose for ~5k edges by default
    let mut h = 70usize;
    let mut repeats = 10usize;
    for a in &args[1..] {
        if let Some(val) = a.strip_prefix("--w=") {
            if let Ok(v) = val.parse() {
                w = v;
            }
        } else if let Some(val) = a.strip_prefix("--h=") {
            if let Ok(v) = val.parse() {
                h = v;
            }
        } else if let Some(val) = a.strip_prefix("--repeats=") {
            if let Ok(v) = val.parse() {
                repeats = v;
            }
        }
    }

    let mut g = build_grid_graph(w, h);

    // Warm caches once, then measure a full recompute on an already-cached graph.
    let _ = g.bench_recompute_regions_full();
    let t0 = Instant::now();
    let _ = g.bench_recompute_regions_full();
    let full_geo_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // Typical incremental: move a single node in place, recompute geometry
    let mut inc_geo_times: Vec<f64> = Vec::new();
    let mid_i = w / 2;
    let mid_j = h / 2;
    let idx = (mid_j * (w + 1) + mid_i) as u32;
    for r in 0..repeats {
        if let Some((x, y)) = g.get_node(idx) {
            let dx = ((r % 5) as f32 - 2.0) * 0.7;
            let dy = ((r % 7) as f32 - 3.0) * 0.5;
            g.move_node(idx, x + dx, y + dy);
        }
        let t_geo = Instant::now();
        let _ = g.bench_recompute_regions_incremental();
        inc_geo_times.push(t_geo.elapsed().as_secs_f64() * 1000.0);
    }
    let avg_inc_geo = if inc_geo_times.is_empty() {
        0.0
    } else {
        inc_geo_times.iter().sum::<f64>() / (inc_geo_times.len() as f64)
    };

    println!(
        "grid={}x{} edges≈{} full_geo_ms={:.3} avg_inc_geo_ms={:.3}",
        w,
        h,
        (w + 1) * h + (h + 1) * w,
        full_geo_ms,
        avg_inc_geo
    );
}
