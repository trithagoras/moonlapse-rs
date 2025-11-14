use std::{
    io::{stdout},
    time::Duration,
};

use ratatui::{crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
}, style::Stylize};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame, Terminal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    Game,
    Inventory,
    Log,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut focus = Focus::Game;

    // Table state for inventory selection
    let mut inv_state = TableState::default();
    inv_state.select(Some(0));

    // Chat scroll offset
    let mut chat_scroll = 0usize;
    let mut input = String::new();

    // Mock data
    let inventory = vec![
        ("Rusty Sword", 1),
        ("Health Potion", 3),
        ("Iron Shield", 1),
        ("Torch", 5),
        ("Old Key", 1),
    ];
    let mut chats = vec![
        "[Alyssa]: Welcome to The Forgotten Moors!".to_string(),
        "[Borin]: Stay close, traveler.".to_string(),
        "[System]: You feel a chill down your spine...".to_string(),
        "[Cedric]: Anyone found the key yet?".to_string(),
        "[Alyssa]: Not yet!".to_string(),
        "[System]: A distant howl echoes...".to_string(),
    ];

    let mut chat_active = false;

    loop {
        terminal.draw(|f| {
            draw_ui(f, focus, &inventory, &mut inv_state, &chats, chat_scroll, &input, chat_active);
        })?;

        // Poll for input at 30 FPS (33ms)
        if event::poll(Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Char('q') if !chat_active => break, // ignore quit while typing

                    // panel focus keys (disabled when typing chat)
                    KeyCode::Char('1') if !chat_active => {
                        focus = Focus::Game;
                    }
                    KeyCode::Char('2') if !chat_active => {
                        focus = Focus::Inventory;
                    }
                    KeyCode::Char('3') if !chat_active => {
                        focus = Focus::Log;
                    }

                    // === ENTER and ESC ===
                    KeyCode::Enter => {
                        if chat_active {
                            // send message
                            if !input.is_empty() {
                                chats.push(format!("[Jon]: {}", input));
                                input.clear();
                            }
                            chat_active = false;
                        } else {
                            chat_active = true;
                        }
                    }
                    KeyCode::Esc if chat_active => {
                        // cancel chat input
                        chat_active = false;
                    }

                    // === Chat typing ===
                    KeyCode::Char(c) if chat_active => input.push(c),
                    KeyCode::Backspace if chat_active => {
                        input.pop();
                    }

                    // === Inventory scrolling (only when not typing) ===
                    KeyCode::Down if focus == Focus::Inventory && !chat_active => {
                        let i = inv_state.selected().unwrap_or(0);
                        let next = (i + 1).min(inventory.len() - 1);
                        inv_state.select(Some(next));
                    }
                    KeyCode::Up if focus == Focus::Inventory && !chat_active => {
                        let i = inv_state.selected().unwrap_or(0);
                        let prev = i.saturating_sub(1);
                        inv_state.select(Some(prev));
                    }

                    // === Log scrolling (only when not typing) ===
                    KeyCode::Up if focus == Focus::Log && !chat_active => {
                        let visible_lines = 11;
                        let max_scroll = chats.len().saturating_sub(visible_lines);
                        if chat_scroll < max_scroll {
                            chat_scroll += 1;
                        }
                    }
                    KeyCode::Down if focus == Focus::Log && !chat_active => {
                        if chat_scroll > 0 {
                            chat_scroll -= 1;
                        }
                    }

                    _ => {}
                },
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn draw_ui(
    f: &mut Frame,
    focus: Focus,
    inventory: &Vec<(&str, i32)>,
    inv_state: &mut TableState,
    chats: &Vec<String>,
    chat_scroll: usize,
    input: &str,
    chat_active: bool
) {
    let root = f.area();

    // Vertical layout: [top row] [log]
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(15)])
        .split(root);

    // Top row: game + inventory side by side
    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(layout[0]);

    draw_game(f, top_row[0], focus == Focus::Game);
    draw_inventory(f, top_row[1], inventory, inv_state, focus == Focus::Inventory);
    draw_log(f, layout[1], chats, chat_scroll, input, focus == Focus::Log, chat_active);
}

fn draw_game(f: &mut Frame, area: Rect, focused: bool) {
    let title = "[1] The Forgotten Moors";

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(if focused { Color::Blue } else { Color::default() }))
        .title_style(Style::default().fg(if focused { Color::Blue } else { Color::default() }));

    let inner = block.inner(area);
    let mut lines: Vec<Line> = Vec::new();

    let width = inner.width.min(30);
    let height = inner.height.min(15);

    let center_x = width / 2;
    let center_y = height / 2;

    for y in 0..height {
        let mut row = String::new();
        for x in 0..width {
            if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
                row.push('#');
            } else if x == center_x && y == center_y {
                row.push('@');
            } else {
                row.push('.');
            }
            row.push(' '); // spacing between cells
        }
        lines.push(Line::from(row));
    }

    let map = Paragraph::new(lines).block(block);
    f.render_widget(map, area);
}

fn draw_inventory(
    f: &mut Frame,
    area: Rect,
    items: &Vec<(&str, i32)>,
    state: &mut TableState,
    focused: bool,
) {
    let title = "[2] Inventory";

    let header = Row::new(vec!["Name", "Qty"])
        .style(Style::default().fg(Color::Cyan))
        .bottom_margin(1);

    let rows: Vec<Row> = items
        .iter()
        .map(|(name, qty)| Row::new(vec![Cell::from(*name), Cell::from(qty.to_string())]))
        .collect();

    let selected_style = if focused {Style::default()
        .bg(Color::Blue)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)} else {Style::default()};

    let t = Table::new(rows, [Constraint::Percentage(70), Constraint::Percentage(30)])
        .header(header)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(if focused { Color::Blue } else { Color::default() }))
            .title_style(Style::default().fg(if focused { Color::Blue } else { Color::default() }))
        )
        .row_highlight_style(selected_style);

    f.render_stateful_widget(t, area, state);
}

fn draw_log(
    f: &mut Frame,
    area: Rect,
    chats: &Vec<String>,
    scroll: usize,
    input: &str,
    focused: bool,
    chat_active: bool
) {
    // Outer bordered block (single frame for entire log area)
    let block = Block::default()
        .borders(Borders::ALL)
        .title("[3] Log")
        .border_style(Style::default().fg(if focused { Color::Blue } else { Color::Reset }))
        .title_style(Style::default().fg(if focused { Color::Blue } else { Color::Reset }));

    let inner = block.inner(area);

    // Split inner area into [chat_area, input_line]
    let layout = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            Constraint::Min(1),  // chat area fills all space above input
            Constraint::Length(1), // 1 line for input
        ])
        .split(inner);

    // --- Chat area ---
    let visible_lines = layout[0].height.saturating_sub(1) as usize;
    let total_lines = chats.len();

    // Ensure we can't scroll beyond the top
    let max_scroll = total_lines.saturating_sub(visible_lines);
    let scroll = scroll.min(max_scroll);

    // Collect lines for visible region
    let lines: Vec<Line> = chats
        .iter()
        .skip(total_lines.saturating_sub(visible_lines + scroll))
        .take(visible_lines)
        .map(|s| Line::from(s.clone()))
        .collect();

    let chat_paragraph = Paragraph::new(lines);
    f.render_widget(block, area); // draw the outer frame
    f.render_widget(chat_paragraph, layout[0]); // draw chat inside

    // --- Input line (fixed to bottom) ---
    let cursor = Span::styled(" ", Style::default().bg(if chat_active {Color::White} else {Color::default()}));
    let input_line = Line::from(vec![
        Span::styled("Say: ", Style::default().fg(Color::Yellow)),
        Span::raw(input),
        // Span::styled("[ENTER] ", Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)),
        cursor,
    ]);

    let input_paragraph = Paragraph::new(input_line);
    f.render_widget(input_paragraph, layout[1]);
}


