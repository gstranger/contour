export type Ok<T> = { ok: true; value: T };
export type Err = { ok: false; error: { code: string; message: string; data?: any } };
export type Result<T> = Ok<T> | Err;

// Minimal Graph subset with strict methods (non-exhaustive)
export declare class Graph {
  constructor();
  geom_version(): number;
  // Strict variants
  add_node_res(x: number, y: number): Result<number>;
  move_node_res(id: number, x: number, y: number): Result<boolean>;
  get_node_res(id: number): Result<[number, number]>;
  add_edge_res(a: number, b: number): Result<number>;
  remove_edge_res(id: number): Result<boolean>;
  pick_res(x: number, y: number, tol: number): Result<null | { kind: 'node'|'edge'|'handle', [k: string]: number }>;
  set_edge_cubic_res(id: number, p1x: number, p1y: number, p2x: number, p2y: number): Result<boolean>;
  set_edge_line_res(id: number): Result<boolean>;
  get_handles_res(id: number): Result<[number, number, number, number]>;
  set_handle_pos_res(id: number, end: 0|1, x: number, y: number): Result<boolean>;
  set_handle_mode_res(id: number, mode: 0|1|2): Result<boolean>;
  bend_edge_to_res(id: number, t: number, tx: number, ty: number, stiffness: number): Result<boolean>;
  get_regions_res(): Result<Array<{ key: number; area: number; filled: boolean; color?: [number,number,number,number]; points: number[] }>>;
  toggle_region_res(key: number): Result<boolean>;
  set_region_fill_res(key: number, filled: boolean): Result<boolean>;
  set_region_color_res(key: number, r: number, g: number, b: number, a: number): Result<boolean>;
  set_flatten_tolerance_res(tol: number): Result<boolean>;
  add_polyline_edge_res(a: number, b: number, points: Float32Array): Result<number>;
  set_edge_polyline_res(id: number, points: Float32Array): Result<boolean>;
  get_polyline_points_res(id: number): Result<Float32Array>;
  add_svg_path_res(d: string): Result<number>;
  to_svg_paths_res(): Result<string[]>;
}

