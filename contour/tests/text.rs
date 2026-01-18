//! Integration tests for text elements.

use contour::Graph;

#[test]
fn test_add_text_label() {
    let mut g = Graph::new();

    // Add a simple text label
    let text_id = g.add_text("Hello, World!", 100.0, 50.0);
    assert_eq!(text_id, 0);

    // Verify it exists
    let text = g.get_text(text_id);
    assert!(text.is_some());

    let text = text.unwrap();
    assert_eq!(text.content, "Hello, World!");
    assert_eq!(text.position.x, 100.0);
    assert_eq!(text.position.y, 50.0);
}

#[test]
fn test_add_text_box() {
    let mut g = Graph::new();

    // Add a text box
    let text_id = g.add_text_box("Lorem ipsum dolor sit amet", 10.0, 20.0, 200.0, 100.0);
    assert_eq!(text_id, 0);

    // Verify it exists
    let text = g.get_text(text_id);
    assert!(text.is_some());

    let text = text.unwrap();
    assert_eq!(text.content, "Lorem ipsum dolor sit amet");
}

#[test]
fn test_text_count_and_ids() {
    let mut g = Graph::new();

    assert_eq!(g.text_count(), 0);

    g.add_text("One", 0.0, 0.0);
    g.add_text("Two", 10.0, 0.0);
    g.add_text("Three", 20.0, 0.0);

    assert_eq!(g.text_count(), 3);

    let ids = g.get_text_ids();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&0));
    assert!(ids.contains(&1));
    assert!(ids.contains(&2));
}

#[test]
fn test_remove_text() {
    let mut g = Graph::new();

    let t1 = g.add_text("First", 0.0, 0.0);
    let t2 = g.add_text("Second", 10.0, 0.0);

    assert_eq!(g.text_count(), 2);

    // Remove first text
    assert!(g.remove_text(t1));
    assert_eq!(g.text_count(), 1);
    assert!(g.get_text(t1).is_none());
    assert!(g.get_text(t2).is_some());

    // Try to remove again (should fail)
    assert!(!g.remove_text(t1));

    // Remove non-existent
    assert!(!g.remove_text(999));
}

#[test]
fn test_set_text_properties() {
    let mut g = Graph::new();

    let text_id = g.add_text("Test", 0.0, 0.0);

    // Set content
    assert!(g.set_text_content(text_id, "Updated"));
    assert_eq!(g.get_text(text_id).unwrap().content, "Updated");

    // Set position
    assert!(g.set_text_position(text_id, 50.0, 75.0));
    let pos = &g.get_text(text_id).unwrap().position;
    assert_eq!(pos.x, 50.0);
    assert_eq!(pos.y, 75.0);

    // Set rotation
    assert!(g.set_text_rotation(text_id, std::f32::consts::PI / 4.0));
    assert!((g.get_text(text_id).unwrap().rotation - std::f32::consts::PI / 4.0).abs() < 0.0001);
}

#[test]
fn test_set_text_font() {
    let mut g = Graph::new();

    let text_id = g.add_text("Test", 0.0, 0.0);

    // Set font
    assert!(g.set_text_font(text_id, "Arial", 24.0));
    let style = &g.get_text(text_id).unwrap().style;
    assert_eq!(style.font_family, "Arial");
    assert_eq!(style.font_size, 24.0);

    // Set weight
    assert!(g.set_text_font_weight(text_id, 700));
    assert_eq!(g.get_text(text_id).unwrap().style.font_weight, 700);

    // Weight clamping
    assert!(g.set_text_font_weight(text_id, 1000));
    assert_eq!(g.get_text(text_id).unwrap().style.font_weight, 900);
}

#[test]
fn test_set_text_colors() {
    let mut g = Graph::new();

    let text_id = g.add_text("Test", 0.0, 0.0);

    // Set fill color
    assert!(g.set_text_fill_color(text_id, 255, 0, 0, 255));
    let fill = g.get_text(text_id).unwrap().style.fill_color;
    assert!(fill.is_some());
    let fill = fill.unwrap();
    assert_eq!(fill.r, 255);
    assert_eq!(fill.g, 0);
    assert_eq!(fill.b, 0);
    assert_eq!(fill.a, 255);

    // Clear fill color
    assert!(g.clear_text_fill_color(text_id));
    assert!(g.get_text(text_id).unwrap().style.fill_color.is_none());

    // Set stroke
    assert!(g.set_text_stroke_color(text_id, 0, 0, 255, 128));
    assert!(g.set_text_stroke_width(text_id, 2.0));
    let style = &g.get_text(text_id).unwrap().style;
    assert!(style.stroke_color.is_some());
    assert_eq!(style.stroke_width, 2.0);
}

#[test]
fn test_convert_text_types() {
    let mut g = Graph::new();

    let text_id = g.add_text("Test", 0.0, 0.0);

    // Convert to box
    assert!(g.convert_text_to_box(text_id, 200.0, 100.0));

    // Convert back to label
    assert!(g.convert_text_to_label(text_id));
}

#[test]
fn test_text_json_roundtrip() {
    let mut g = Graph::new();

    // Add various text elements
    let t1 = g.add_text("Label text", 100.0, 50.0);
    let t2 = g.add_text_box("Box text", 10.0, 20.0, 200.0, 100.0);

    // Set some properties
    g.set_text_font(t1, "Helvetica", 18.0);
    g.set_text_fill_color(t1, 0, 128, 255, 255);

    // Serialize
    let json = g.to_json_value();

    // Create new graph and deserialize
    let mut g2 = Graph::new();
    assert!(g2.from_json_value(json));

    // Verify texts
    assert_eq!(g2.text_count(), 2);

    let text1 = g2.get_text(t1).unwrap();
    assert_eq!(text1.content, "Label text");
    assert_eq!(text1.style.font_family, "Helvetica");
    assert_eq!(text1.style.font_size, 18.0);

    let text2 = g2.get_text(t2).unwrap();
    assert_eq!(text2.content, "Box text");
}

#[test]
fn test_invalid_text_operations() {
    let mut g = Graph::new();

    // Operations on non-existent text
    assert!(!g.set_text_content(999, "test"));
    assert!(!g.set_text_position(999, 0.0, 0.0));
    assert!(!g.set_text_rotation(999, 0.0));
    assert!(!g.set_text_font(999, "Arial", 12.0));
    assert!(!g.remove_text(999));
    assert!(g.get_text(999).is_none());
}
