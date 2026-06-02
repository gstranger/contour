export type Ok<T> = { ok: true; value: T };
export type Err = { ok: false; error: { code: string; message: string; data?: any } };
export type Result<T> = Ok<T> | Err;

/**
 * Snapshot of changes since a caller-supplied version, returned by `get_dirty`.
 * If `full` is true the caller should rebuild instead of applying the diff.
 * Consumers should call `get_dirty` BEFORE any call that triggers a region
 * pass within a frame, since region recompute clears the window.
 */
export interface DirtyDiff {
  current_ver: number;
  since_ver: number;
  full: boolean;
  nodes_added: number[];
  nodes_removed: number[];
  nodes_moved: number[];
  edges_added: number[];
  edges_removed: number[];
  edges_modified: number[];
  bbox: [number, number, number, number] | null;
}

// Minimal Graph subset with strict methods (non-exhaustive)
export declare class Graph {
  constructor();
  geom_version(): number;
  get_dirty(since: number): DirtyDiff;
  get_dirty_res(since: number): Result<DirtyDiff>;
  dirty_reset(): void;
  dirty_reset_res(): Result<boolean>;
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
  // Undo/redo
  begin_undo_group_res(label: string): Result<boolean>;
  end_undo_group_res(): Result<{ committed: boolean; depth_was_zero: boolean }>;
  undo_res(): Result<{ label: string; depth_remaining: number }>;
  redo_res(): Result<{ label: string; depth_remaining: number }>;
  can_undo_res(): Result<boolean>;
  can_redo_res(): Result<boolean>;
  undo_clear_res(): Result<boolean>;
  undo_depth_res(): Result<{ undo: number; redo: number }>;

  // Text metrics and layout
  path_length(edge_ids: Uint32Array): number;
  path_length_res(edge_ids: Uint32Array): Result<number>;
  measure_text(id: number): TextMeasureResult | null;
  measure_text_res(id: number): Result<TextMeasureResult>;
  set_text_metrics(id: number, metrics: TextMetrics): boolean;
  set_text_metrics_res(id: number, metrics: TextMetrics): Result<boolean>;
  get_text_char_positions(id: number): TextCharPosition[] | null;
  get_text_char_positions_res(id: number): Result<TextCharPosition[]>;
  get_text_hit(id: number, x: number, y: number): [number, number] | null;
  get_text_hit_res(id: number, x: number, y: number): Result<[number, number]>;
  get_text_selection_bounds(id: number, start: number, end: number): [number, number, number, number][] | null;
  get_text_selection_bounds_res(id: number, start: number, end: number): Result<[number, number, number, number][]>;
  sample_path_point(edge_ids: Uint32Array, distance: number): { x: number; y: number; angle: number } | null;
  sample_path_point_res(edge_ids: Uint32Array, distance: number): Result<{ x: number; y: number; angle: number }>;
}

export interface TextMeasureResult {
  needs_measure: boolean;
  content: string;
  style: {
    font_family: string;
    font_size: number;
    font_weight: number;
    font_style: number;
    fill_color: { r: number; g: number; b: number; a: number } | null;
    stroke_color: { r: number; g: number; b: number; a: number } | null;
    stroke_width: number;
    letter_spacing: number;
    line_height: number;
  };
  char_widths: number[];
  line_height: number;
  ascent: number;
  descent: number;
  total_width: number;
}

export interface TextMetrics {
  char_widths: number[];
  line_height: number;
  ascent: number;
  descent: number;
  total_width: number;
}

export interface TextCharPosition {
  x: number;
  y: number;
  w: number;
  char_index: number;
  line_index: number;
}

