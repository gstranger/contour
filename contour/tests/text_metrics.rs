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