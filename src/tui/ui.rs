use crate::tui::{ActivePanel, App, TxRow};
use crate::types::TransactionType;
use chrono::Local;
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

/// Format a monetary value using accounting notation:
///   positive →  "  1,234.56"
///   negative →  " (1,234.56)"
/// The result is right-aligned within `width` characters.
fn fmt_amount(value: f64, width: usize) -> String {
    if value < 0.0 {
        let inner = format!("{:.2}", value.abs());
        let with_parens = format!("({})", inner);
        format!("{:>width$}", with_parens, width = width)
    } else {
        format!("{:>width$.2}", value, width = width)
    }
}

/// Render the full TUI layout.
pub fn render(frame: &mut Frame, app: &mut App) {
    // Ensure tx_offset keeps the cursor visible
    let tx_panel_height = (frame.area().height as usize).saturating_sub(5);
    if app.tx_cursor >= app.tx_offset + tx_panel_height {
        app.tx_offset = app
            .tx_cursor
            .saturating_sub(tx_panel_height)
            .saturating_add(1);
    } else if app.tx_cursor < app.tx_offset {
        app.tx_offset = app.tx_cursor;
    }

    // Overall layout: top bar, main panels, stats bar, help bar
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // top bar
            Constraint::Min(0),    // panels
            Constraint::Length(1), // stats
            Constraint::Length(1), // help
        ])
        .split(frame.area());

    // ---------- Top bar ----------
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),
            Constraint::Fill(1),
            Constraint::Length(12),
        ])
        .split(layout[0]);

    let top_style = Style::default().bg(Color::Cyan).fg(Color::White);
    let left = Paragraph::new("MoneyManager")
        .style(top_style.add_modifier(Modifier::BOLD))
        .alignment(Alignment::Left);
    let center = Paragraph::new(app.backup.file_path_hint())
        .style(top_style)
        .alignment(Alignment::Center);
    let right = Paragraph::new(Local::now().format("%d/%m/%Y").to_string())
        .style(top_style)
        .alignment(Alignment::Right);
    frame.render_widget(left, top_chunks[0]);
    frame.render_widget(center, top_chunks[1]);
    frame.render_widget(right, top_chunks[2]);

    // ---------- Main panels ----------
    let panel_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(layout[1]);

    // ---- Accounts panel (left) ----
    let accounts_title = "ACCOUNTS";
    let accounts_block = Block::default()
        .borders(Borders::ALL)
        .title(accounts_title)
        .border_style(if app.active_panel == ActivePanel::Accounts {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        });

    // Build list items
    let mut list_items: Vec<ListItem> = Vec::new();
    for (idx, row) in app.account_rows.iter().enumerate() {
        let item = if row.is_header() {
            // Header style: grey background, bold white
            let span = Span::styled(
                row.display_name(),
                Style::default()
                    .fg(Color::White)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            );
            ListItem::new(Line::from(span))
        } else {
            // Account row — name left-aligned, balance right-aligned to panel edge
            let name = row.display_name();
            let bal = row.balance().unwrap_or(0.0);
            let bal_color = if bal >= 0.0 { Color::Blue } else { Color::Red };

            // panel_chunks[0].width includes the 2-char borders
            let inner_width = (panel_chunks[0].width as usize).saturating_sub(2);
            let bal_width = 12usize; // e.g. "-12,345.67" fits in 12 chars
            let name_width = inner_width.saturating_sub(bal_width + 1); // +1 for separator space

            // Truncate name if too long
            let name_display: String = if name.len() > name_width {
                name.chars()
                    .take(name_width.saturating_sub(1))
                    .collect::<String>()
                    + "…"
            } else {
                name.to_string()
            };

            let bal_str = fmt_amount(bal, bal_width);
            let name_span = Span::raw(format!(
                "{:<name_width$}",
                name_display,
                name_width = name_width
            ));
            let bal_span = Span::styled(bal_str, Style::default().fg(bal_color));
            ListItem::new(Line::from(vec![name_span, Span::raw(" "), bal_span]))
        };
        // Highlight selected row (skip headers)
        let highlighted = idx == app.account_cursor && !row.is_header();
        let final_item = if highlighted {
            item.style(Style::default().bg(Color::Yellow))
        } else {
            item
        };
        list_items.push(final_item);
    }

    // Determine visible slice based on height
    let panel_height = (panel_chunks[0].height as usize).saturating_sub(2);
    let mut start = 0usize;
    if app.account_cursor >= panel_height && panel_height > 0 {
        start = app.account_cursor + 1 - panel_height;
    }
    let visible_items: Vec<ListItem> = list_items
        .into_iter()
        .skip(start)
        .take(panel_height)
        .collect();

    let accounts_list = List::new(visible_items).block(accounts_block);
    frame.render_widget(accounts_list, panel_chunks[0]);

    // ---- Transactions panel (right) ----
    let tx_title = "TRANSACTIONS";
    let tx_block = Block::default()
        .borders(Borders::ALL)
        .title(tx_title)
        .border_style(if app.active_panel == ActivePanel::Transactions {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        });

    // Column widths (must sum <= inner panel width)
    // inner = panel_chunks[1].width - 2 (borders)
    let inner_w = (panel_chunks[1].width as usize).saturating_sub(2);
    const COL_DATE: usize = 12;
    const COL_TYPE: usize = 10;
    const COL_AMT: usize = 13;
    const COL_CAT: usize = 18;
    const COL_SEP: usize = 4; // 4 × 1-space column gaps
    let col_desc = inner_w.saturating_sub(COL_DATE + COL_TYPE + COL_AMT + COL_CAT + COL_SEP);

    // Sticky column-header row
    let col_header = ListItem::new(Line::from(vec![
        Span::styled(
            format!("{:<COL_DATE$} ", "Date"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<COL_TYPE$} ", "Type"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>COL_AMT$} ", "Amount"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<COL_CAT$} ", "Category"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<col_desc$}", "Description"),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ]));

    // Build list items
    let mut tx_items: Vec<ListItem> = vec![col_header];
    for (idx, tx_row) in app.tx_rows.iter().enumerate() {
        let item = match tx_row {
            TxRow::DateHeader { date } => {
                // Full-width divider: "── 2026-05-06 ──────..."
                let label = format!("── {} ", date);
                let filler = "─".repeat(inner_w.saturating_sub(label.len()));
                ListItem::new(Line::from(Span::styled(
                    format!("{}{}", label, filler),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )))
            }
            TxRow::Tx { tx } => {
                let (type_str, type_color) = match tx.transaction_type {
                    TransactionType::Income => ("Income", Color::Green),
                    TransactionType::Expense => ("Expense", Color::Red),
                    TransactionType::TransferOut => ("Transfer", Color::Cyan),
                    TransactionType::TransferIn => ("TransferIn", Color::Cyan),
                };
                let amt = fmt_amount(tx.amount, COL_AMT);
                let cat_raw = match (&tx.parent_category_name, &tx.category_name) {
                    (Some(p), Some(s)) => format!("{}/{}", p, s),
                    (None, Some(c)) => c.clone(),
                    _ => "-".to_string(),
                };
                // Truncate category if too long
                let cat = if cat_raw.chars().count() > COL_CAT {
                    cat_raw
                        .chars()
                        .take(COL_CAT.saturating_sub(1))
                        .collect::<String>()
                        + "…"
                } else {
                    cat_raw
                };
                let desc_raw = tx.description();
                let desc = if desc_raw.chars().count() > col_desc {
                    desc_raw
                        .chars()
                        .take(col_desc.saturating_sub(1))
                        .collect::<String>()
                        + "…"
                } else {
                    desc_raw
                };

                let highlight = idx == app.tx_cursor;
                let hl = if highlight {
                    Style::default().bg(Color::Yellow).fg(Color::Black)
                } else {
                    Style::default()
                };

                ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<COL_DATE$} ", tx.date), hl),
                    Span::styled(
                        format!("{:<COL_TYPE$} ", type_str),
                        if highlight {
                            hl
                        } else {
                            Style::default().fg(type_color)
                        },
                    ),
                    Span::styled(format!("{:>COL_AMT$} ", amt), hl),
                    Span::styled(
                        format!("{:<COL_CAT$} ", cat),
                        if highlight {
                            hl
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(format!("{:<col_desc$}", desc), hl),
                ]))
            }
        };
        tx_items.push(item);
    }

    // Visible slice (+1 for sticky header already prepended)
    let tx_panel_height = (panel_chunks[1].height as usize).saturating_sub(2); // borders only
    let visible_tx: Vec<ListItem> = tx_items
        .into_iter()
        .enumerate()
        .filter_map(|(i, item)| {
            if i == 0 {
                return Some(item);
            } // always keep col header
            let data_idx = i - 1;
            if data_idx >= app.tx_offset && data_idx < app.tx_offset + tx_panel_height - 1 {
                Some(item)
            } else {
                None
            }
        })
        .collect();

    let tx_list_widget = List::new(visible_tx).block(tx_block);
    frame.render_widget(tx_list_widget, panel_chunks[1]);

    // ---------- Stats bar ----------
    let mut assets_sum = 0.0f64;
    let mut liabilities_sum = 0.0f64;
    for row in &app.account_rows {
        if let Some(bal) = row.balance() {
            if bal >= 0.0 {
                assets_sum += bal;
            } else {
                liabilities_sum += bal;
            }
        }
    }
    let net = assets_sum + liabilities_sum;
    let stats_text = format!(
        "Assets: {}  Liabilities: {}  Net: {}",
        fmt_amount(assets_sum, 15),
        fmt_amount(liabilities_sum, 15),
        fmt_amount(net, 15),
    );
    let stats_par = Paragraph::new(stats_text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White))
        .alignment(Alignment::Left);
    frame.render_widget(stats_par, layout[2]);

    // ---------- Help bar ----------
    let help_text = "↑↓ navigate | Tab switch panel | Enter select account | Esc clear | q quit";
    let help_par = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Left);
    frame.render_widget(help_par, layout[3]);
}
