use contour::Graph;
use std::time::Instant;

fn build_grid_graph(edges_target: usize) -> Graph {
    let mut g = Graph::new();
    // Create a grid of nodes; connect horizontally until ~edges_target
    let w = 120usize; let h = 120usize; // up to 14k edges available
    let mut nodes = Vec::with_capacity(w*h);
    for j in 0..h { for i in 0..w { nodes.push(g.add_node(i as f32 * 8.0, j as f32 * 6.0)); } }
    let mut ecount = 0usize;
    'outer: for j in 0..h { for i in 0..(w-1) { let a=nodes[j*w+i]; let b=nodes[j*w+i+1]; g.add_edge(a,b); ecount+=1; if ecount>=edges_target { break 'outer; } } }
    g
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() { return 0.0; }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len()-1)]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut edges = 5000usize;
    let mut picks = 10000usize;
    let mut tol = 3.0f32;
    let mut assert_ms: Option<f64> = None;
    for a in &args[1..] {
        if let Some(val)=a.strip_prefix("--edges=") { if let Ok(v)=val.parse() { edges=v; } }
        else if let Some(val)=a.strip_prefix("--picks=") { if let Ok(v)=val.parse() { picks=v; } }
        else if let Some(val)=a.strip_prefix("--tol=") { if let Ok(v)=val.parse() { tol=v; } }
        else if let Some(val)=a.strip_prefix("--assert-ms=") { if let Ok(v)=val.parse() { assert_ms=Some(v); } }
    }

    let mut g = build_grid_graph(edges);
    // Warm-up build of index
    let _ = g.pick(1.0, 1.0, tol);

    // Generate pick points cycling across the grid
    let mut times_ms: Vec<f64> = Vec::with_capacity(picks);
    let start_all = Instant::now();
    let mut hits = 0usize;
    for k in 0..picks {
        let x = (k % 100) as f32 * 8.0 + 1.1;
        let y = ((k/100) % 100) as f32 * 6.0 + 0.9;
        let t0 = Instant::now();
        if g.pick(x, y, tol).is_some() { hits += 1; }
        let dt = t0.elapsed().as_secs_f64() * 1000.0;
        times_ms.push(dt);
    }
    let dur_all = start_all.elapsed().as_secs_f64() * 1000.0;
    times_ms.sort_by(|a,b| a.partial_cmp(b).unwrap());
    let med = percentile(&times_ms, 0.5);
    let p90 = percentile(&times_ms, 0.9);
    let p99 = percentile(&times_ms, 0.99);
    println!("edges={} picks={} tol={} hits={} total_ms={:.3} median_ms={:.4} p90_ms={:.4} p99_ms={:.4}", edges, picks, tol, hits, dur_all, med, p90, p99);
    if let Some(th) = assert_ms { if med > th { eprintln!("FAIL: median {:.4} ms > threshold {:.3} ms", med, th); std::process::exit(1); } }
}

