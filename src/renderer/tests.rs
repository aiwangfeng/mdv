use super::table::truncate_text;
use super::{render_nodes, render_viewport, IMAGE_RENDER_HEIGHT};
use crate::config;
use crate::parser::DocNode;

#[test]
fn reserves_space_for_images() {
    config::load().unwrap();
    let rendered = render_nodes(
        &[DocNode::Image {
            src: "img.png".to_string(),
            alt: "alt".to_string(),
        }],
        80,
        80,
    );

    assert_eq!(rendered.image_positions[0].0, 0);
    assert_eq!(rendered.lines.len(), IMAGE_RENDER_HEIGHT);
}

#[test]
fn tracks_rendered_line_start_per_node() {
    config::load().unwrap();
    let rendered = render_nodes(
        &[
            DocNode::Heading {
                level: 1,
                text: "Heading".to_string(),
            },
            DocNode::Blank,
            DocNode::CodeBlock {
                language: None,
                code: "a\nb\n".to_string(),
            },
        ],
        40,
        40,
    );

    assert_eq!(rendered.node_line_starts, vec![0, 1, 2]);
}

#[test]
fn keeps_blockquote_images_and_headings_rendered() {
    config::load().unwrap();
    let rendered = render_nodes(
        &[DocNode::BlockQuote(vec![
            DocNode::Heading {
                level: 2,
                text: "Quoted".to_string(),
            },
            DocNode::Blank,
            DocNode::Image {
                src: "img.png".to_string(),
                alt: "diagram".to_string(),
            },
        ])],
        40,
        40,
    );

    assert!(rendered.lines[1]
        .spans
        .iter()
        .any(|span| span.content.contains("Quoted")));
    assert_eq!(rendered.image_positions.len(), 1);
    // Image line should be after top border (line 0), heading (line 1), blank (line 2) -> line 3
    assert_eq!(rendered.image_positions[0].0, 3);
}

#[test]
fn wraps_cjk_text_by_display_width() {
    config::load().unwrap();
    let rendered = render_nodes(
        &[DocNode::Paragraph(vec![crate::parser::InlineSpan::Text(
            "你好世界".to_string(),
        )])],
        4,
        4,
    );

    assert!(rendered.lines.len() >= 2);
}

#[test]
fn truncates_utf8_text_without_breaking_encoding() {
    let truncated = truncate_text("你好世界", 3);

    assert!(truncated.is_char_boundary(truncated.len()));
    assert_eq!(unicode_width::UnicodeWidthStr::width(truncated.as_str()), 3);
    assert_eq!(truncated, "你…");
}

#[test]
fn blockquote_box_widths_match() {
    config::load().unwrap();
    let width: u16 = 40;
    let rendered = render_nodes(
        &[DocNode::BlockQuote(vec![DocNode::Paragraph(vec![
            crate::parser::InlineSpan::Text("Hello world".to_string()),
        ])])],
        width,
        width,
    );

    assert_eq!(rendered.lines.len(), 3);
    for (i, line) in rendered.lines.iter().enumerate() {
        let w: usize = line
            .spans
            .iter()
            .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
            .sum();
        assert_eq!(w, width as usize, "line {i} width mismatch");
    }
    assert!(rendered.lines[0].spans[0].content.starts_with("╭"));
    assert!(rendered.lines[0].spans[0].content.ends_with("╮"));
    assert!(rendered.lines[2].spans[0].content.starts_with("╰"));
    assert!(rendered.lines[2].spans[0].content.ends_with("╯"));
}

#[test]
fn blockquote_renders_in_buffer() {
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        text::Text,
        widgets::{Paragraph, Widget, Wrap},
    };

    config::load().unwrap();
    let width: u16 = 40;
    let height: u16 = 5;
    let rendered = render_nodes(
        &[DocNode::BlockQuote(vec![DocNode::Paragraph(vec![
            crate::parser::InlineSpan::Text("Hello world".to_string()),
        ])])],
        width,
        width,
    );

    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    let paragraph = Paragraph::new(Text::from(rendered.lines.clone())).wrap(Wrap { trim: false });
    paragraph.render(area, &mut buf);

    for y in 0..height {
        let mut row = String::new();
        let mut x = 0u16;
        while x < width {
            let cell = &buf[(x, y)];
            let sym = cell.symbol();
            row.push_str(sym);
            let sw = unicode_width::UnicodeWidthStr::width(sym) as u16;
            x += if sw > 0 { sw } else { 1 };
        }
        eprintln!("row {y}: |{row}|");
    }

    assert_eq!(buf[(0, 0)].symbol(), "╭");
    assert_eq!(buf[(width - 1, 0)].symbol(), "╮");
    assert_eq!(buf[(width - 1, 1)].symbol(), "│");
    assert_eq!(buf[(width - 1, 2)].symbol(), "╯");
}

#[test]
fn table_borders_align_with_columns() {
    use unicode_width::UnicodeWidthStr;

    config::load().unwrap();

    let width: u16 = 100;

    let headers = vec![
        "作品名称".to_string(),
        "在线地址".to_string(),
        "上线日期".to_string(),
    ];
    let rows = vec![vec![
        "逍遥自在轩".to_string(),
        "https://niceshare.site".to_string(),
        "2024-04-26".to_string(),
    ]];

    let rendered = render_nodes(&[DocNode::Table { headers, rows }], width, width);

    for (i, line) in rendered.lines.iter().enumerate() {
        let line_width: usize = line
            .spans
            .iter()
            .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
            .sum();
        eprintln!(
            "line {i} total_width={line_width} spans={:?}",
            line.spans
                .iter()
                .map(|s| (
                    UnicodeWidthStr::width(s.content.as_ref()),
                    s.content.as_ref().to_string()
                ))
                .collect::<Vec<_>>()
        );
    }

    // Verify header and data row widths match borders
    let header_row = &rendered.lines[1];
    let top_border = &rendered.lines[0];
    let header_total: usize = header_row
        .spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let border_total: usize = top_border
        .spans
        .iter()
        .map(|s| UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    eprintln!(
        "header_total={}, border_total={}",
        header_total, border_total
    );
    assert_eq!(
        header_total, border_total,
        "Header row width should match top border width"
    );
}

#[test]
fn renders_code_block_with_and_without_language() {
    config::load().unwrap();
    // without language
    let r = render_nodes(
        &[DocNode::CodeBlock {
            language: None,
            code: "echo hi".into(),
        }],
        80,
        80,
    );
    let lines_str: Vec<String> = r.lines.iter().map(|l| l.to_string()).collect();
    let combined = lines_str.join("\n");
    assert!(
        combined.contains("echo hi"),
        "code content should appear: {combined}"
    );

    // with language
    let r2 = render_nodes(
        &[DocNode::CodeBlock {
            language: Some("rust".into()),
            code: "fn main() {}".into(),
        }],
        80,
        80,
    );
    let lines_str2: Vec<String> = r2.lines.iter().map(|l| l.to_string()).collect();
    let combined2 = lines_str2.join("\n");
    assert!(
        combined2.contains("fn main()"),
        "code content should appear: {combined2}"
    );
}

#[test]
fn renders_unordered_list_items() {
    config::load().unwrap();
    let nodes = vec![
        DocNode::ListItem {
            depth: 0,
            ordered: false,
            number: None,
            children: vec![crate::parser::InlineSpan::Text("apple".into())],
        },
        DocNode::ListItem {
            depth: 0,
            ordered: false,
            number: None,
            children: vec![crate::parser::InlineSpan::Text("banana".into())],
        },
    ];
    let r = render_nodes(&nodes, 80, 80);
    let lines_str: Vec<String> = r.lines.iter().map(|l| l.to_string()).collect();
    let combined = lines_str.join("|");
    assert!(
        combined.contains("•"),
        "unordered list should have bullet: {combined}"
    );
    assert!(combined.contains("apple"), "first item: {combined}");
    assert!(combined.contains("banana"), "second item: {combined}");
}

#[test]
fn renders_ordered_list_with_numbers() {
    config::load().unwrap();
    let nodes = vec![
        DocNode::ListItem {
            depth: 0,
            ordered: true,
            number: Some(1),
            children: vec![crate::parser::InlineSpan::Text("first".into())],
        },
        DocNode::ListItem {
            depth: 0,
            ordered: true,
            number: Some(2),
            children: vec![crate::parser::InlineSpan::Text("second".into())],
        },
    ];
    let r = render_nodes(&nodes, 80, 80);
    let lines_str: Vec<String> = r.lines.iter().map(|l| l.to_string()).collect();
    let combined = lines_str.join("|");
    assert!(
        combined.contains("1.") || combined.contains("1 "),
        "ordered list should show number: {combined}"
    );
    assert!(combined.contains("first"), "first item: {combined}");
}

#[test]
fn renders_thematic_break_as_line() {
    config::load().unwrap();
    let r = render_nodes(&[DocNode::Rule], 40, 40);
    let lines_str: Vec<String> = r.lines.iter().map(|l| l.to_string()).collect();
    let combined = lines_str.join("\n");
    assert!(!combined.is_empty(), "rule should produce output");
    // Should contain a horizontal line character (─ or similar)
    assert!(
        combined.contains('─') || combined.contains('━'),
        "should have a horizontal line: {combined}"
    );
}

#[test]
fn renders_blank_as_empty_line() {
    config::load().unwrap();
    let r = render_nodes(&[DocNode::Blank], 80, 80);
    assert!(
        !r.lines.is_empty(),
        "blank should produce at least one line"
    );
    let lines_str: Vec<String> = r.lines.iter().map(|l| l.to_string()).collect();
    assert!(
        lines_str.iter().all(|l| l.trim().is_empty()),
        "blank lines should be empty: {lines_str:?}"
    );
}

#[test]
fn renders_heading_levels() {
    config::load().unwrap();
    for level in 1..=3 {
        let r = render_nodes(
            &[DocNode::Heading {
                level,
                text: format!("Heading {level}"),
            }],
            80,
            80,
        );
        let lines_str: Vec<String> = r.lines.iter().map(|l| l.to_string()).collect();
        let combined = lines_str.join("\n");
        assert!(
            combined.contains(&format!("Heading {level}")),
            "heading {level} text should appear: {combined}"
        );
    }
}

#[test]
fn wraps_long_headings() {
    config::load().unwrap();
    let text = "This is a very long heading that should definitely wrap across multiple lines because the width is very small".to_string();
    let rendered = render_nodes(
        &[DocNode::Heading {
            level: 1,
            text: text.clone(),
        }],
        20,
        20,
    );
    assert!(
        rendered.lines.len() > 1,
        "heading should wrap to multiple lines"
    );
    let height = super::measure::measure_node_height(&DocNode::Heading { level: 1, text }, 20);
    assert_eq!(
        rendered.lines.len(),
        height,
        "measured height should match rendered line count"
    );
}

#[test]
fn render_viewport_only_renders_requested_range() {
    config::load().unwrap();
    let nodes = vec![
        DocNode::Heading {
            level: 1,
            text: "Line 1".to_string(),
        },
        DocNode::Blank,
        DocNode::Heading {
            level: 2,
            text: "Line 3".to_string(),
        },
        DocNode::Blank,
        DocNode::Paragraph(vec![crate::parser::InlineSpan::Text("Line 5".to_string())]),
        DocNode::Blank,
        DocNode::Heading {
            level: 3,
            text: "Line 7".to_string(),
        },
    ];
    let node_line_starts = vec![0, 1, 2, 3, 4, 5, 6];

    // Render entire document for reference
    let full = render_nodes(&nodes, 80, 80);
    assert_eq!(full.lines.len(), 7);

    // Render just the middle portion (lines 2..=4)
    let partial = render_viewport(&nodes, &node_line_starts, 2, 5, 80, 80);
    assert_eq!(partial.lines.len(), 3, "should render 3 lines (2, 3, 4)");
    assert!(
        partial.lines[0].to_string().contains("Line 3"),
        "first rendered line should be 'Line 3': got {:?}",
        partial.lines[0].to_string()
    );
    assert!(
        partial.lines[2].to_string().contains("Line 5"),
        "third rendered line should be 'Line 5': got {:?}",
        partial.lines[2].to_string()
    );

    // Render lines beyond the document (should clamp gracefully)
    let beyond = render_viewport(&nodes, &node_line_starts, 5, 20, 80, 80);
    assert_eq!(beyond.lines.len(), 2, "only 2 lines remain (5, 6)");
}

#[test]
fn render_viewport_non_trivial_heights() {
    config::load().unwrap();
    let nodes = vec![
        DocNode::CodeBlock {
            language: None,
            code: "line 1\nline 2\nline 3".to_string(), // height is 3 + 2 = 5 lines
        },
        DocNode::Paragraph(vec![crate::parser::InlineSpan::Text(
            "Para line 6".to_string(),
        )]), // height is 1 line
    ];
    let node_line_starts = vec![0, 5];

    // Render entire document for reference
    let full = render_nodes(&nodes, 80, 80);
    assert_eq!(full.lines.len(), 6);

    // Render starting in the middle of the code block (line 2) to the end (line 6)
    let partial = render_viewport(&nodes, &node_line_starts, 2, 6, 80, 80);
    // Should return exactly 4 lines (index 2, 3, 4 of codeblock + index 5 of paragraph)
    assert_eq!(partial.lines.len(), 4);

    // Check that first line is indeed inside the code block content, not the top border.
    // The full render lines:
    // 0: top border (lang label inside)
    // 1: line 1
    // 2: line 2
    // 3: line 3
    // 4: bottom border
    // 5: Para line 6
    assert!(partial.lines[0].to_string().contains("line 2"));
    assert!(partial.lines[1].to_string().contains("line 3"));
    assert!(
        partial.lines[2].to_string().contains("╯") || partial.lines[2].to_string().contains("┘")
    );
    assert_eq!(partial.lines[3].to_string().trim(), "Para line 6");
}

#[test]
fn test_all_node_types_height_matches_rendering() {
    config::load().unwrap();
    let markdown = r#"
# Heading 1
## Heading 2 with some longer text to test wrapping behavior at smaller widths

Some paragraph text that contains **bold** and *italic* and `inline code` and a [link](https://example.com).
This paragraph also has a soft break
and a hard break\
at the end of a line.

- Short list item
- Longer list item that will wrap when the width is restricted to a small value
- List item with `inline code` inside it

1. Ordered item 1
2. Ordered item 2 with some more text to wrap

```rust
fn main() {
    println!("Hello, world!");
}
```

```
No language block
```

> Blockquote content
> With multiple lines
> And maybe a heading inside it:
> ### Quoted heading
> And a blank line
>
> Inside blockquote

| Col 1 | Col 2 |
|---|---|
| val 1 | val 2 |
| longer value | another val |

---

![Alt text](img.png)
"#;
    let doc = crate::parser::parse(markdown);
    for width in (10..=120).step_by(5) {
        for (i, node) in doc.nodes.iter().enumerate() {
            let measured = super::measure::measure_node_height(node, width as u16);
            let rendered = render_nodes(std::slice::from_ref(node), width as u16, width as u16)
                .lines
                .len();
            assert_eq!(
                measured, rendered,
                "Width {}: Node at index {} ({:?}) has mismatched height: measured={}, rendered={}",
                width, i, node, measured, rendered
            );
        }
    }
}

#[test]
fn test_heading_height_mismatch_fuzz() {
    config::load().unwrap();
    let words = vec![
        "word",
        "longerword",
        "verylongwordthatmightwrapitself",
        "a",
        "b",
        "c",
    ];
    for level in 1..=6 {
        for num_words in 1..20 {
            let mut text = String::new();
            for j in 0..num_words {
                if j > 0 {
                    text.push(' ');
                }
                text.push_str(words[j % words.len()]);
            }
            let node = DocNode::Heading {
                level,
                text: text.clone(),
            };
            for width in 5..=150 {
                let measured = super::measure::measure_node_height(&node, width as u16);
                let rendered =
                    render_nodes(std::slice::from_ref(&node), width as u16, width as u16)
                        .lines
                        .len();
                assert_eq!(
                    measured,
                    rendered,
                    "Heading level {}, width {}: text='{}' has mismatched height: measured={}, rendered={}",
                    level, width, text, measured, rendered
                );
            }
        }
    }
}

#[test]
fn test_cumulative_line_starts_match_rendering() {
    config::load().unwrap();
    let markdown = r#"
# Heading 1
## Heading 2 with some longer text to test wrapping behavior at smaller widths

Some paragraph text that contains **bold** and *italic* and `inline code` and a [link](https://example.com).
This paragraph also has a soft break
and a hard break\
at the end of a line.

- Short list item
- Longer list item that will wrap when the width is restricted to a small value
- List item with `inline code` inside it

1. Ordered item 1
2. Ordered item 2 with some more text to wrap

```rust
fn main() {
    println!("Hello, world!");
}
```

```
No language block
```

> Blockquote content
> With multiple lines
> And maybe a heading inside it:
> ### Quoted heading
> And a blank line
>
> Inside blockquote

| Col 1 | Col 2 |
|---|---|
| val 1 | val 2 |
| longer value | another val |

---

![Alt text](img.png)
"#;
    let doc = crate::parser::parse(markdown);
    for width in (10..=120).step_by(5) {
        let node_heights = super::measure::measure_nodes(&doc.nodes, width as u16);
        let computed_starts = super::measure::compute_line_starts(&node_heights);
        let rendered = render_nodes(&doc.nodes, width as u16, width as u16);
        assert_eq!(
            computed_starts, rendered.node_line_starts,
            "Width {}: cumulative line starts mismatched",
            width
        );
        let total_measured = node_heights.iter().sum::<usize>();
        assert_eq!(
            total_measured,
            rendered.lines.len(),
            "Width {}: total height mismatched",
            width
        );
    }
}

#[test]
fn test_cjk_width_calculations() {
    crate::width::set_cjk_width(false);
    assert_eq!(crate::width::char_width('·'), 1);
    assert_eq!(crate::width::char_width('✓'), 1);
    assert_eq!(crate::width::char_width('中'), 2);
    assert_eq!(crate::width::str_width("2024 · 车规"), 11); // 4 + 1 + 1 + 1 + 4 = 11

    crate::width::set_cjk_width(true);
    assert_eq!(crate::width::char_width('·'), 2);
    assert_eq!(crate::width::char_width('✓'), 2);
    assert_eq!(crate::width::char_width('中'), 2);
    assert_eq!(crate::width::str_width("2024 · 车规"), 12); // 4 + 1 + 2 + 1 + 4 = 12
}

#[test]
fn test_code_block_alignment_cjk() {
    config::load().unwrap();
    crate::width::set_cjk_width(true);
    
    let code = "2024 · 车规 ✓ |\n     · 沟槽 ✓ |";
    let r = render_nodes(
        &[DocNode::CodeBlock {
            language: None,
            code: code.to_string(),
        }],
        80,
        80,
    );
    
    // Check that the vertical bars are aligned on the same visual column
    // The lines are:
    // 0: top border
    // 1: first line of code
    // 2: second line of code
    // 3: bottom border
    assert_eq!(r.lines.len(), 4);
    
    // In ratatui Line, each span contains content. Let's find the column index of '|' visually.
    let line1 = &r.lines[1];
    let line2 = &r.lines[2];
    
    let width1: usize = line1.spans.iter().map(|s| crate::width::str_width(s.content.as_ref())).sum();
    let width2: usize = line2.spans.iter().map(|s| crate::width::str_width(s.content.as_ref())).sum();
    
    // Both lines must have exactly the same display width (80) due to padding inside the code block border
    assert_eq!(width1, 80);
    assert_eq!(width2, 80);
    
    // Let's check the position of '|' inside the spans
    let get_bar_pos = |line: &ratatui::text::Line| -> usize {
        let mut pos = 0;
        for span in &line.spans {
            if let Some(idx) = span.content.find('|') {
                pos += crate::width::str_width(&span.content[..idx]);
                break;
            }
            pos += crate::width::str_width(span.content.as_ref());
        }
        pos
    };
    
    let pos1 = get_bar_pos(line1);
    let pos2 = get_bar_pos(line2);
    assert_eq!(pos1, pos2, "Vertical bars should align exactly in CJK mode");
}

