use contour::Graph;
use contour::model::TextMetrics;

#[test]
fn test_measure_text_returns_needs_measure_for_new_text() {
    let mut g = Graph::new();
    let id = g.add_text("Hello", 0.0, 0.0);

    let result = g.measure_text(id);
    assert!(result.is_some());
    let needs = result.unwrap();
    assert!(needs.needs_measure, "new text should need measurement");
    assert_eq!(needs.content, "Hello");
}

#[test]
fn test_set_metrics_then_measure_returns_cached() {
    let mut g = Graph::new();
    let id = g.add_text("Hi", 0.0, 0.0);

    let metrics = TextMetrics {
        char_widths: vec![8.0, 5.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 13.0,
    };
    assert!(g.set_text_metrics(id, metrics));

    let result = g.measure_text(id);
    assert!(result.is_some());
    let cached = result.unwrap();
    assert!(!cached.needs_measure, "should be cached after set_text_metrics");
    assert_eq!(cached.total_width, 13.0);
}

#[test]
fn test_editing_content_invalidates_cache() {
    let mut g = Graph::new();
    let id = g.add_text("Hi", 0.0, 0.0);

    let metrics = TextMetrics {
        char_widths: vec![8.0, 5.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 13.0,
    };
    g.set_text_metrics(id, metrics);

    // Edit content
    g.set_text_content(id, "Hello");

    let result = g.measure_text(id);
    assert!(result.is_some());
    assert!(result.unwrap().needs_measure, "should need re-measure after edit");
}

#[test]
fn test_measure_text_returns_none_for_invalid_id() {
    let g = Graph::new();
    assert!(g.measure_text(999).is_none());
}

#[test]
fn test_set_text_metrics_returns_false_for_invalid_id() {
    let mut g = Graph::new();
    assert!(!g.set_text_metrics(999, TextMetrics::default()));
}

#[test]
fn test_get_text_char_positions_after_set_metrics() {
    let mut g = Graph::new();
    let id = g.add_text("AB", 10.0, 20.0);

    let metrics = TextMetrics {
        char_widths: vec![10.0, 6.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 16.0,
    };
    g.set_text_metrics(id, metrics);

    let positions = g.get_text_char_positions(id);
    assert!(positions.is_some(), "should return positions after set_text_metrics");
    let pos = positions.unwrap();
    assert_eq!(pos.len(), 2);
    assert_eq!(pos[0].char_index, 0);
    assert_eq!(pos[0].line_index, 0);
    assert!((pos[0].w - 10.0).abs() < 0.01);
    assert_eq!(pos[1].char_index, 1);
    // Second char starts after first char width + letter spacing
    assert!((pos[1].x - 10.0).abs() < 0.01);
}

#[test]
fn test_get_text_hit_finds_correct_char() {
    let mut g = Graph::new();
    let id = g.add_text("AB", 0.0, 0.0);

    let metrics = TextMetrics {
        char_widths: vec![10.0, 6.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 16.0,
    };
    g.set_text_metrics(id, metrics);

    // Hit first char center
    let hit = g.get_text_hit(id, 5.0, 14.0);
    assert!(hit.is_some());
    let (ci, li) = hit.unwrap();
    assert_eq!(ci, 0);
    assert_eq!(li, 0);

    // Hit second char center
    let hit2 = g.get_text_hit(id, 13.0, 14.0);
    assert!(hit2.is_some());
    assert_eq!(hit2.unwrap().0, 1);

    // Far away still returns nearest char
    let hit3 = g.get_text_hit(id, 100.0, 100.0);
    assert!(hit3.is_some());
}

#[test]
fn test_get_text_hit_returns_none_without_cache() {
    let mut g = Graph::new();
    let id = g.add_text("AB", 0.0, 0.0);
    // No set_text_metrics called
    assert!(g.get_text_hit(id, 5.0, 14.0).is_none());
}

#[test]
fn test_get_text_selection_bounds_returns_rects() {
    let mut g = Graph::new();
    let id = g.add_text("ABC", 10.0, 20.0);

    let metrics = TextMetrics {
        char_widths: vec![10.0, 10.0, 10.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 30.0,
    };
    g.set_text_metrics(id, metrics);

    // Select first two chars
    let rects = g.get_text_selection_bounds(id, 0, 2);
    assert!(rects.is_some());
    let r = rects.unwrap();
    assert_eq!(r.len(), 2, "should have 2 rects for 2 chars");
    // Each rect is [x, y, w, h]
    assert!((r[0][0] - 10.0).abs() < 0.01); // x = text.position.x + pos.x
    assert!((r[0][2] - 10.0).abs() < 0.01); // w = char width
}

#[test]
fn test_get_text_selection_bounds_reversed_range() {
    let mut g = Graph::new();
    let id = g.add_text("ABC", 0.0, 0.0);

    let metrics = TextMetrics {
        char_widths: vec![10.0, 10.0, 10.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 30.0,
    };
    g.set_text_metrics(id, metrics);

    // end < start should still work (normalized internally)
    let rects = g.get_text_selection_bounds(id, 2, 0);
    assert!(rects.is_some());
    let r = rects.unwrap();
    assert_eq!(r.len(), 2);
}

#[test]
fn test_get_text_char_positions_handles_text_box_layout() {
    let mut g = Graph::new();
    let id = g.add_text_box("AB", 0.0, 0.0, 30.0, 50.0);

    let metrics = TextMetrics {
        char_widths: vec![10.0, 10.0],
        line_height: 19.2,
        ascent: 14.0,
        descent: 5.0,
        total_width: 20.0,
    };
    g.set_text_metrics(id, metrics);

    let positions = g.get_text_char_positions(id);
    assert!(positions.is_some());
    let pos = positions.unwrap();
    assert_eq!(pos.len(), 2, "text box with 2 chars should have 2 positions");
    assert_eq!(pos[0].line_index, 0);
}