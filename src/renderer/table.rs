//! Table rendering: column width computation, cell truncation, border drawing.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::{
    display_width, TABLE_BOTTOM_LEFT, TABLE_BOTTOM_MID, TABLE_BOTTOM_RIGHT, TABLE_CELL_PADDING,
    TABLE_CELL_PADDING_TOTAL, TABLE_MID_LEFT, TABLE_MID_MID, TABLE_MID_RIGHT, TABLE_MIN_COL_WIDTH,
    TABLE_TOP_LEFT, TABLE_TOP_MID, TABLE_TOP_RIGHT, TABLE_VERTICAL_BORDER,
};
use crate::theme::Theme;

pub(super) fn render_table(
    lines: &mut Vec<Line<'static>>,
    headers: &[String],
    rows: &[Vec<String>],
    width: usize,
) {
    if headers.is_empty() {
        return;
    }
    let ncols = headers.len();
    let col_widths = compute_col_widths(headers, rows, width, ncols);

    let border = Theme::table_border();
    let h_style = Theme::table_header();

    // Top border
    lines.push(table_top_border(&col_widths, border));

    // Header row
    let mut header_spans = vec![Span::styled(TABLE_VERTICAL_BORDER, border)];
    for (i, h) in headers.iter().enumerate() {
        let w = col_widths[i];
        let content = truncate_text(h, w);
        let content_width = display_width(&content);
        header_spans.push(Span::styled(
            format!("{}{}", " ".repeat(TABLE_CELL_PADDING), content),
            h_style,
        ));
        let remaining = w.saturating_sub(content_width) + TABLE_CELL_PADDING;
        header_spans.push(Span::styled(
            format!("{}{}", " ".repeat(remaining), TABLE_VERTICAL_BORDER),
            border,
        ));
    }
    lines.push(Line::from(header_spans));

    // Header separator
    lines.push(table_separator(&col_widths, border));

    // Data rows
    for (ri, row) in rows.iter().enumerate() {
        let row_style = if ri % 2 == 0 {
            Theme::table_row_even()
        } else {
            Theme::table_row_odd()
        };
        let mut row_spans = vec![Span::styled(TABLE_VERTICAL_BORDER, border)];
        for (i, cell) in row.iter().enumerate() {
            let w = col_widths.get(i).copied().unwrap_or(TABLE_MIN_COL_WIDTH);
            let content = truncate_text(cell, w);
            let content_width = display_width(&content);
            row_spans.push(Span::styled(
                format!("{}{}", " ".repeat(TABLE_CELL_PADDING), content),
                row_style,
            ));
            let remaining = w.saturating_sub(content_width) + TABLE_CELL_PADDING;
            row_spans.push(Span::styled(
                format!("{}{}", " ".repeat(remaining), TABLE_VERTICAL_BORDER),
                border,
            ));
        }
        // pad missing cells
        for &w in col_widths.iter().take(ncols).skip(row.len()) {
            let empty_cell = " ".repeat(TABLE_CELL_PADDING_TOTAL + w);
            row_spans.push(Span::styled(empty_cell, row_style));
            row_spans.push(Span::styled(TABLE_VERTICAL_BORDER, border));
        }
        lines.push(Line::from(row_spans));
    }

    // Bottom border
    lines.push(table_bottom_border(&col_widths, border));
}

fn compute_col_widths(
    headers: &[String],
    rows: &[Vec<String>],
    max_width: usize,
    ncols: usize,
) -> Vec<usize> {
    let vert_border_w = display_width(TABLE_VERTICAL_BORDER);
    let overhead = (ncols + 1) * vert_border_w + ncols * TABLE_CELL_PADDING_TOTAL;

    let mut natural_widths: Vec<usize> = headers
        .iter()
        .map(|h| display_width(h).max(TABLE_MIN_COL_WIDTH))
        .collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < natural_widths.len() {
                natural_widths[i] = natural_widths[i].max(display_width(cell));
            }
        }
    }

    let avail = max_width.saturating_sub(overhead);
    let total_natural: usize = natural_widths.iter().sum();

    if total_natural <= avail {
        return natural_widths;
    }

    if avail < ncols * TABLE_MIN_COL_WIDTH {
        let equal = (avail / ncols).max(TABLE_MIN_COL_WIDTH);
        return vec![equal; ncols];
    }

    let mut widths = natural_widths.clone();
    let total_excess: usize = widths
        .iter()
        .map(|&w| w.saturating_sub(TABLE_MIN_COL_WIDTH))
        .sum();

    if total_excess == 0 {
        return widths;
    }

    let shrinkage_needed = total_natural.saturating_sub(avail);
    let scale = (shrinkage_needed as f64 / total_excess as f64).min(1.0);

    for w in widths.iter_mut() {
        let excess = w.saturating_sub(TABLE_MIN_COL_WIDTH);
        let shrink = (excess as f64 * scale).floor() as usize;
        *w = (*w - shrink).max(TABLE_MIN_COL_WIDTH);
    }

    let total_after: usize = widths.iter().sum();
    if total_after > avail {
        let remaining = total_after - avail;
        for i in 0..remaining {
            let idx = i % ncols;
            widths[idx] = widths[idx].saturating_sub(1).max(TABLE_MIN_COL_WIDTH);
        }
    }

    widths
}

pub(super) fn truncate_text(s: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = crate::width::char_width(c);
        if w + cw > max_w {
            let ellipsis_width = crate::width::char_width('…');
            while !result.is_empty() && w + ellipsis_width > max_w {
                if let Some(removed) = result.pop() {
                    w = w.saturating_sub(crate::width::char_width(removed));
                }
            }
            if w + ellipsis_width <= max_w {
                result.push('…');
            }
            break;
        }
        result.push(c);
        w += cw;
    }
    result
}

fn build_table_border(
    col_widths: &[usize],
    corners: (&str, &str, &str),
    style: Style,
) -> Line<'static> {
    let mut s = String::from(corners.0);
    for (i, &w) in col_widths.iter().enumerate() {
        s.push_str(&"─".repeat(w + TABLE_CELL_PADDING_TOTAL));
        if i < col_widths.len() - 1 {
            s.push_str(corners.1);
        } else {
            s.push_str(corners.2);
        }
    }
    Line::from(Span::styled(s, style))
}

fn table_top_border(col_widths: &[usize], style: Style) -> Line<'static> {
    build_table_border(
        col_widths,
        (TABLE_TOP_LEFT, TABLE_TOP_MID, TABLE_TOP_RIGHT),
        style,
    )
}

fn table_separator(col_widths: &[usize], style: Style) -> Line<'static> {
    build_table_border(
        col_widths,
        (TABLE_MID_LEFT, TABLE_MID_MID, TABLE_MID_RIGHT),
        style,
    )
}

fn table_bottom_border(col_widths: &[usize], style: Style) -> Line<'static> {
    build_table_border(
        col_widths,
        (TABLE_BOTTOM_LEFT, TABLE_BOTTOM_MID, TABLE_BOTTOM_RIGHT),
        style,
    )
}
