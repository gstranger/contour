Handle Constraint Semantics

Modes
- Free: handles are independent; no coupling after edits.
- Mirrored: handles are opposite directions with equal lengths.
- Aligned: handles are opposite directions; lengths may differ.

Application
- Post-bend: Apply least-squares update to both handles, then enforce the mode constraint. The edited side is chosen by the curve parameter t (<=0.5 → start; >0.5 → end) to avoid visual jumps while dragging.
- set_handle_pos: Update the specified end’s handle to the new offset; re-enforce the mode constraint using the edited end as the driver.
- set_handle_mode: Switch modes and immediately re-normalize handles to satisfy constraints.
- move_node: Handle offsets are relative to nodes, so constraints remain satisfied when endpoints move.
- convert line→cubic: Create symmetric, opposite handles along the segment; zero-length edges become no-ops.

Epsilon
- Constraint tolerance: EPS_CONSTRAINT (1e-3) for tests and comparisons.
- Degenerate guards: see docs/epsilons.md for EPS_LEN, EPS_POS, etc.

Notes
- When the edited end is known (set_handle_pos or bend with t-based heuristic), Mirrored uses that end’s length as the target; otherwise an average length is used for smoothness.
- Aligned preserves the unedited end’s length and sets the opposite direction.

