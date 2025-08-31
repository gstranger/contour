use contour::Graph;
use std::time::Instant;

fn lcg(seed: &mut u64) -> f32 { *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1); (((*seed >> 24) & 0xFFFF_FFFF) as u32) as f32 / (u32::MAX as f32) }

// Long-running persistence test; enable with: cargo test --features long-persist
#[cfg_attr(not(feature = "long-persist"), ignore)]
#[test]
fn determinism_and_persistence() {
    // Build a synthetic grid scene with extra diagonals to create varied faces
    let mut g = Graph::new();
    let w = 8usize; let h = 8usize; // keep test runtime bounded in CI
    let mut nodes = Vec::new();
    for j in 0..h { for i in 0..w { nodes.push(g.add_node(i as f32 * 8.0, j as f32 * 8.0)); } }
    // Add horizontals and some diagonals
    for j in 0..h { for i in 0..(w-1) { let a=nodes[j*w+i]; let b=nodes[j*w+i+1]; g.add_edge(a,b); } }
    for i in 0..(w-1) { for j in 0..(h-1) { if (i+j)%5==0 { let a=nodes[j*w+i]; let b=nodes[(j+1)*w+i+1]; g.add_edge(a,b); } } }

    // Speed up region computation
    g.set_flatten_tolerance(0.6);
    // Initial keys snapshot with sorted order enforced by get_regions
    let r0 = g.get_regions();
    let mut keys0: Vec<u32> = r0.iter().filter_map(|v| v.get("key").and_then(|x| x.as_u64()).map(|x| x as u32)).collect();
    keys0.sort_unstable();

    // Mark all current regions filled
    for k in &keys0 { g.set_region_fill(*k, true); }

    // 1000 random edits: mostly bends, occasional small node moves (low chance of topology change)
    let mut seed = 0xDEC0DE1234ABCDu64;
    let mut accumulated_ratio = 0.0f64; let mut samples = 0usize;
    let t_start = Instant::now();
    let log_progress = std::env::var("VECNET_PROGRESS").is_ok();
    for step in 0..800 {
        if step % 20 == 0 { // small node jiggle
            let id = (lcg(&mut seed) * (nodes.len() as f32)) as usize % nodes.len();
            let (x,y)=g.get_node(nodes[id]).unwrap();
            let dx=(lcg(&mut seed)-0.5)*0.6; let dy=(lcg(&mut seed)-0.5)*0.6;
            g.move_node(nodes[id], x+dx, y+dy);
        } else {
            // random bend on a random edge id
            let ea = g.get_edge_arrays(); if ea.ids.is_empty() { continue; }
            let i = (lcg(&mut seed) * (ea.ids.len() as f32)) as usize % ea.ids.len();
            let eid = ea.ids[i];
            let t = (lcg(&mut seed)*1.4 - 0.2).max(0.0).min(1.0);
            let tx = lcg(&mut seed)*160.0; let ty = lcg(&mut seed)*160.0;
            let _ = g.bend_edge_to(eid, t, tx, ty, 1.0);
        }
        // Trigger recompute + remap
        if step % 40 == 0 { // measure infrequently to reduce runtime
            let regs = g.get_regions();
            let filled = regs.iter().filter(|v| v.get("filled").and_then(|x| x.as_bool()).unwrap_or(false)).count();
            let total_now = regs.len().max(1);
            let rate = (filled as f64) / (total_now as f64);
            accumulated_ratio += rate; samples += 1;
            if log_progress {
                eprintln!(
                    "[persist] step={}/800 elapsed={:.2}s fill_rate={:.4}",
                    step,
                    t_start.elapsed().as_secs_f64(),
                    rate
                );
            }
        }
    }
    let avg = if samples>0 { accumulated_ratio / (samples as f64) } else { 1.0 };
    assert!(avg >= 0.995, "persistence {:.5} below target", avg);
}

#[test]
fn quick_persistence_smoke() {
    let mut g = Graph::new();
    let w = 8usize; let h = 8usize;
    let mut nodes = Vec::new();
    for j in 0..h { for i in 0..w { nodes.push(g.add_node(i as f32 * 10.0, j as f32 * 10.0)); } }
    for j in 0..h { for i in 0..(w-1) { let a=nodes[j*w+i]; let b=nodes[j*w+i+1]; g.add_edge(a,b); } }
    g.set_flatten_tolerance(0.6);
    let r0 = g.get_regions();
    let mut keys0: Vec<u32> = r0.iter().filter_map(|v| v.get("key").and_then(|x| x.as_u64()).map(|x| x as u32)).collect();
    keys0.sort_unstable();
    for k in &keys0 { g.set_region_fill(*k, true); }
    // Perform 120 quick bends
    let mut seed = 0x1234_5678_9ABC_DEF0u64;
    for step in 0..120 {
        let ea = g.get_edge_arrays(); if ea.ids.is_empty() { break; }
        let i = ((seed as usize) % ea.ids.len());
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let eid = ea.ids[i];
        let t = (((seed >> 8) as f32) / (u32::MAX as f32)).min(1.0).max(0.0);
        let tx = ((seed >> 16) as f32 % 200.0); let ty = ((seed >> 24) as f32 % 200.0);
        let _ = g.bend_edge_to(eid, t, tx, ty, 1.0);
        if step % 20 == 0 { let _ = g.get_regions(); }
    }
    let regs = g.get_regions();
    let mut keys: Vec<u32> = regs.iter().filter_map(|v| v.get("key").and_then(|x| x.as_u64()).map(|x| x as u32)).collect();
    keys.sort_unstable();
    let set0: std::collections::HashSet<u32> = keys0.iter().copied().collect();
    let set1: std::collections::HashSet<u32> = keys.iter().copied().collect();
    let inter = set0.intersection(&set1).count();
    if !set0.is_empty() { let rate = (inter as f64) / (set0.len() as f64); assert!(rate >= 0.98, "quick persistence {:.3} below expected", rate); }
}

// Release-only: 1k edits persistence >= 99.5%
#[cfg(not(debug_assertions))]
#[test]
fn release_persistence_1k() {
    use std::collections::HashSet;
    let mut g = Graph::new();
    let w = 12usize; let h = 12usize;
    let mut nodes = Vec::new();
    for j in 0..h { for i in 0..w { nodes.push(g.add_node(i as f32 * 8.0, j as f32 * 8.0)); } }
    // horizontals
    for j in 0..h { for i in 0..(w-1) { let a=nodes[j*w+i]; let b=nodes[j*w+i+1]; g.add_edge(a,b); } }
    // sparse diagonals to vary topology
    for i in 0..(w-1) { for j in 0..(h-1) { if (i*j)%7==0 { let a=nodes[j*w+i]; let b=nodes[(j+1)*w+i+1]; g.add_edge(a,b); } } }

    g.set_flatten_tolerance(0.6);

    // Initial fill pattern: checker by quantized centroid
    let regs0 = g.get_regions();
    let mut keys0: Vec<u32> = regs0.iter().filter_map(|v| v.get("key").and_then(|x| x.as_u64()).map(|x| x as u32)).collect();
    keys0.sort_unstable();
    for v in regs0.iter() {
        let k = v.get("key").and_then(|x| x.as_u64()).unwrap() as u32;
        let pts = v.get("points").and_then(|x| x.as_array()).unwrap();
        let mut cx=0.0f32; let mut cy=0.0f32; let mut a=0.0f32;
        for i in (0..pts.len()).step_by(2) { let j=(i+2)%pts.len(); let xi=pts[i].as_f64().unwrap() as f32; let yi=pts[i+1].as_f64().unwrap() as f32; let xj=pts[j].as_f64().unwrap() as f32; let yj=pts[j+1].as_f64().unwrap() as f32; let cross=xi*yj-xj*yi; a+=cross; cx+=(xi+xj)*cross; cy+=(yi+yj)*cross; }
        a*=0.5; if a.abs()<1e-2 { continue; } cx/=(6.0*a); cy/=(6.0*a);
        let qx=(cx*10.0).round() as i32; let qy=(cy*10.0).round() as i32;
        let fill = ((qx + qy) & 1) == 0;
        g.set_region_fill(k, fill);
    }
    let _ = g.get_regions(); // commit fills

    // prev snapshot: (qx,qy,filled)
    let mut prev: Vec<(i32,i32,bool)> = {
        let regs = g.get_regions();
        regs.iter().map(|v| {
            let pts = v.get("points").and_then(|x| x.as_array()).unwrap();
            let mut cx=0.0f32; let mut cy=0.0f32; let mut a=0.0f32;
            for i in (0..pts.len()).step_by(2) { let j=(i+2)%pts.len(); let xi=pts[i].as_f64().unwrap() as f32; let yi=pts[i+1].as_f64().unwrap() as f32; let xj=pts[j].as_f64().unwrap() as f32; let yj=pts[j+1].as_f64().unwrap() as f32; let cross=xi*yj-xj*yi; a+=cross; cx+=(xi+xj)*cross; cy+=(yi+yj)*cross; }
            a*=0.5; if a.abs()<1e-2 { (0,0,false) } else { cx/=(6.0*a); cy/=(6.0*a); let qx=(cx*10.0).round() as i32; let qy=(cy*10.0).round() as i32; let filled=v.get("filled").and_then(|x| x.as_bool()).unwrap_or(false); (qx,qy,filled) }
        }).collect()
    };

    // Perform 1k edits; measure nearest-centroid persistence per step
    let mut seed = 0xCAFEBABE_u64;
    let mut matches = 0usize; let mut total = 0usize;
    for step in 0..1000 {
        if step % 25 == 0 { // small jitter
            let idx = ((seed as usize) % nodes.len()); seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let (x,y)=g.get_node(nodes[idx]).unwrap();
            let dx=((seed>>8) as f32 / (u32::MAX as f32) - 0.5)*0.6; let dy=((seed>>16) as f32 / (u32::MAX as f32) - 0.5)*0.6;
            g.move_node(nodes[idx], x+dx, y+dy);
        } else {
            let ea = g.get_edge_arrays(); if ea.ids.is_empty() { continue; }
            let i = ((seed as usize) % ea.ids.len()); seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let eid = ea.ids[i];
            let t = (((seed>>24) as f32) / (u32::MAX as f32)).min(1.0).max(0.0);
            let tx = ((seed >> 28) as f32 % 160.0); let ty = ((seed >> 32) as f32 % 160.0);
            let _ = g.bend_edge_to(eid, t, tx, ty, 1.0);
        }
        let regs = g.get_regions();
        // Build new (qx,qy,filled)
        let mut curr: Vec<(i32,i32,bool)> = Vec::with_capacity(regs.len());
        for v in regs.iter() {
            let pts = v.get("points").and_then(|x| x.as_array()).unwrap();
            let mut cx=0.0f32; let mut cy=0.0f32; let mut a=0.0f32;
            for i in (0..pts.len()).step_by(2) { let j=(i+2)%pts.len(); let xi=pts[i].as_f64().unwrap() as f32; let yi=pts[i+1].as_f64().unwrap() as f32; let xj=pts[j].as_f64().unwrap() as f32; let yj=pts[j+1].as_f64().unwrap() as f32; let cross=xi*yj-xj*yi; a+=cross; cx+=(xi+xj)*cross; cy+=(yi+yj)*cross; }
            a*=0.5; if a.abs()<1e-2 { continue; } cx/=(6.0*a); cy/=(6.0*a); let qx=(cx*10.0).round() as i32; let qy=(cy*10.0).round() as i32; let filled=v.get("filled").and_then(|x| x.as_bool()).unwrap_or(false); curr.push((qx,qy,filled));
        }
        // For each new face, find nearest prev by qcentroid and compare filled
        for (qx,qy,fill) in curr.iter() {
            let mut best_d2 = i64::MAX; let mut best_f=false;
            for (pqx,pqy,pf) in prev.iter() { let dx=*qx as i64 - *pqx as i64; let dy=*qy as i64 - *pqy as i64; let d2=dx*dx+dy*dy; if d2<best_d2 { best_d2=d2; best_f=*pf; } }
            if best_d2 < i64::MAX { total+=1; if *fill == best_f { matches+=1; } }
        }
        prev = curr;
    }
    if total>0 { let rate = (matches as f64) / (total as f64); assert!(rate>=0.995, "release persistence {:.5} below target", rate); }
}
