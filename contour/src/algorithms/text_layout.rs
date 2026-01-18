//! Text box layout algorithm.
//!
//! Handles line breaking and wrapping for text boxes.
//! Character metrics (widths) are provided from JavaScript via font measurement APIs.

use crate::model::{LayoutLine, TextAlign, TextStyle, VerticalAlign};

/// Result of text box layout
#[derive(Debug, Clone)]
pub struct TextBoxLayout {
    /// Lines of laid out text
    pub lines: Vec<LayoutLine>,
    /// Total height of all lines
    pub total_height: f32,
    /// Whether text was truncated
    pub truncated: bool,
}

/// Layout text into a box with line wrapping.
///
/// # Arguments
/// * `content` - The text content to layout
/// * `width` - Box width in pixels
/// * `height` - Box height in pixels
/// * `style` - Text styling (for line height)
/// * `char_widths` - Width of each character (from JS font measurement)
/// * `align` - Horizontal text alignment
/// * `vertical_align` - Vertical text alignment
///
/// # Returns
/// TextBoxLayout with positioned lines
pub fn layout_text_box(
    content: &str,
    width: f32,
    height: f32,
    style: &TextStyle,
    char_widths: &[f32],
    align: TextAlign,
    vertical_align: VerticalAlign,
) -> TextBoxLayout {
    if content.is_empty() || width <= 0.0 {
        return TextBoxLayout {
            lines: Vec::new(),
            total_height: 0.0,
            truncated: false,
        };
    }

    let line_height_px = style.font_size * style.line_height;
    let letter_spacing_px = style.letter_spacing * style.font_size;

    let chars: Vec<char> = content.chars().collect();
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_line_start = 0;
    let mut current_line_width = 0.0;
    let mut last_word_boundary = 0;
    let mut last_word_boundary_width = 0.0;

    for (i, ch) in chars.iter().enumerate() {
        let char_width = char_widths.get(i).copied().unwrap_or(style.font_size * 0.5)
            + letter_spacing_px;

        // Check for newline
        if *ch == '\n' {
            let line_text: String = chars[current_line_start..i].iter().collect();
            lines.push(LayoutLine {
                text: line_text,
                y_offset: 0.0, // Will be calculated later
                x_offset: 0.0, // Will be calculated later
                width: current_line_width,
            });
            current_line_start = i + 1;
            current_line_width = 0.0;
            last_word_boundary = i + 1;
            last_word_boundary_width = 0.0;
            continue;
        }

        // Track word boundaries (spaces)
        if ch.is_whitespace() {
            last_word_boundary = i + 1;
            last_word_boundary_width = current_line_width + char_width;
        }

        // Check if we need to wrap
        if current_line_width + char_width > width && current_line_start < i {
            // Wrap at word boundary if possible
            let wrap_at = if last_word_boundary > current_line_start {
                // Wrap at last word boundary
                let line_text: String =
                    chars[current_line_start..last_word_boundary].iter().collect();
                let line_w = last_word_boundary_width - letter_spacing_px;
                lines.push(LayoutLine {
                    text: line_text.trim_end().to_string(),
                    y_offset: 0.0,
                    x_offset: 0.0,
                    width: line_w.max(0.0),
                });
                last_word_boundary
            } else {
                // Force break in middle of word
                let line_text: String = chars[current_line_start..i].iter().collect();
                lines.push(LayoutLine {
                    text: line_text,
                    y_offset: 0.0,
                    x_offset: 0.0,
                    width: current_line_width - letter_spacing_px,
                });
                i
            };

            current_line_start = wrap_at;
            current_line_width = if wrap_at == i { char_width } else { 0.0 };
            last_word_boundary = wrap_at;
            last_word_boundary_width = 0.0;

            // Recalculate width from wrap_at to current position
            if wrap_at < i {
                for j in wrap_at..=i {
                    if j < char_widths.len() {
                        current_line_width += char_widths[j] + letter_spacing_px;
                    }
                }
            }
        } else {
            current_line_width += char_width;
        }
    }

    // Add final line
    if current_line_start < chars.len() {
        let line_text: String = chars[current_line_start..].iter().collect();
        lines.push(LayoutLine {
            text: line_text,
            y_offset: 0.0,
            x_offset: 0.0,
            width: current_line_width - letter_spacing_px,
        });
    }

    // Calculate total height and check for truncation
    let total_height = lines.len() as f32 * line_height_px;
    let truncated = total_height > height;

    // Calculate y offsets based on vertical alignment
    let content_height = if truncated { height } else { total_height };
    let y_start = match vertical_align {
        VerticalAlign::Top => 0.0,
        VerticalAlign::Middle => (height - content_height) / 2.0,
        VerticalAlign::Bottom => height - content_height,
    };

    // Apply y offsets and x alignment
    for (i, line) in lines.iter_mut().enumerate() {
        line.y_offset = y_start + (i as f32 * line_height_px) + style.font_size;

        line.x_offset = match align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (width - line.width) / 2.0,
            TextAlign::Right => width - line.width,
        };
    }

    TextBoxLayout {
        lines,
        total_height,
        truncated,
    }
}

/// Get positions for each character in a text box layout.
/// Useful for cursor positioning and hit testing.
pub fn get_character_positions(
    layout: &TextBoxLayout,
    char_widths: &[f32],
    letter_spacing: f32,
    font_size: f32,
) -> Vec<(f32, f32, f32)> {
    // Returns (x, y, width) for each character
    let mut positions = Vec::new();
    let mut char_idx = 0;
    let letter_spacing_px = letter_spacing * font_size;

    for line in &layout.lines {
        let mut x = line.x_offset;
        for ch in line.text.chars() {
            let width = char_widths.get(char_idx).copied().unwrap_or(font_size * 0.5);
            positions.push((x, line.y_offset, width));
            x += width + letter_spacing_px;
            char_idx += 1;
        }
        // Account for newline character
        char_idx += 1;
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_style() -> TextStyle {
        TextStyle {
            font_family: "sans-serif".to_string(),
            font_size: 16.0,
            font_weight: 400,
            font_style: crate::model::FontStyle::Normal,
            fill_color: None,
            stroke_color: None,
            stroke_width: 0.0,
            letter_spacing: 0.0,
            line_height: 1.2,
        }
    }

    #[test]
    fn test_single_line() {
        let content = "Hello";
        let style = default_style();
        // Each character is 10px wide
        let char_widths: Vec<f32> = vec![10.0; 5];

        let layout = layout_text_box(
            content,
            200.0,
            100.0,
            &style,
            &char_widths,
            TextAlign::Left,
            VerticalAlign::Top,
        );

        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.lines[0].text, "Hello");
        assert!((layout.lines[0].width - 50.0).abs() < 0.1);
        assert!(!layout.truncated);
    }

    #[test]
    fn test_word_wrap() {
        let content = "Hello World Test";
        let style = default_style();
        // Each character ~10px wide
        let char_widths: Vec<f32> = vec![10.0; content.len()];

        let layout = layout_text_box(
            content,
            60.0, // Force wrapping
            100.0,
            &style,
            &char_widths,
            TextAlign::Left,
            VerticalAlign::Top,
        );

        assert!(layout.lines.len() >= 2);
    }

    #[test]
    fn test_explicit_newline() {
        let content = "Line1\nLine2";
        let style = default_style();
        let char_widths: Vec<f32> = vec![10.0; content.len()];

        let layout = layout_text_box(
            content,
            200.0,
            100.0,
            &style,
            &char_widths,
            TextAlign::Left,
            VerticalAlign::Top,
        );

        assert_eq!(layout.lines.len(), 2);
        assert_eq!(layout.lines[0].text, "Line1");
        assert_eq!(layout.lines[1].text, "Line2");
    }

    #[test]
    fn test_center_alignment() {
        let content = "Hi";
        let style = default_style();
        let char_widths: Vec<f32> = vec![10.0; 2];

        let layout = layout_text_box(
            content,
            100.0,
            50.0,
            &style,
            &char_widths,
            TextAlign::Center,
            VerticalAlign::Top,
        );

        assert_eq!(layout.lines.len(), 1);
        // Text is 20px wide, box is 100px, so x_offset should be 40
        assert!((layout.lines[0].x_offset - 40.0).abs() < 1.0);
    }

    #[test]
    fn test_vertical_center() {
        let content = "Test";
        let style = default_style();
        let char_widths: Vec<f32> = vec![10.0; 4];

        let layout = layout_text_box(
            content,
            100.0,
            100.0,
            &style,
            &char_widths,
            TextAlign::Left,
            VerticalAlign::Middle,
        );

        // Line height is 16 * 1.2 = 19.2, so y_offset should be around (100-19.2)/2 + 16
        assert!(layout.lines[0].y_offset > 30.0);
        assert!(layout.lines[0].y_offset < 70.0);
    }

    #[test]
    fn test_empty_content() {
        let layout = layout_text_box(
            "",
            100.0,
            100.0,
            &default_style(),
            &[],
            TextAlign::Left,
            VerticalAlign::Top,
        );

        assert!(layout.lines.is_empty());
        assert!(!layout.truncated);
    }
}
