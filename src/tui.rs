use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Terminal,
};
use std::time::{Duration, Instant};
use anyhow::Result;
use crate::man_db::ManDb;
use std::sync::Arc;

const PAGE_SIZE: usize = 30;
const LIST_SIZE: usize = 50;
const DEBOUNCE_DELAY_MS: u64 = 150;

pub struct AppState {
    input: String,
    filtered_commands: Arc<Vec<String>>,
    selected_idx: usize,
    man_content: Arc<Vec<String>>,
    scroll: usize,
    focus: Focus,
    man_db: Arc<ManDb>,
    loading: bool,
    search_query: String,
    search_matches: Arc<Vec<usize>>,
    current_match: usize,
    list_scroll: usize,
    last_input_time: Instant,
    pending_man_load: bool,
    visible_list_range: (usize, usize),
}

enum Focus {
    CommandList,
    ManPage,
    Search,
}

pub async fn run_tui(man_db: ManDb) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let man_db = Arc::new(man_db);
    let commands = man_db.get_commands();
    let filtered_commands = Arc::new(commands.clone());

    let mut app = AppState {
        input: String::new(),
        filtered_commands,
        selected_idx: 0,
        man_content: Arc::new(Vec::new()),
        scroll: 0,
        focus: Focus::CommandList,
        man_db: man_db.clone(),
        loading: false,
        search_query: String::new(),
        search_matches: Arc::new(Vec::new()),
        current_match: 0,
        list_scroll: 0,
        last_input_time: Instant::now(),
        pending_man_load: true,
        visible_list_range: (0, 0),
    };

    loop {
        let now = Instant::now();

        // Обработка отложенной загрузки man-страницы
        if app.pending_man_load && app.last_input_time.elapsed() > Duration::from_millis(DEBOUNCE_DELAY_MS) {
            load_current_man_page(&mut app).await;
            app.pending_man_load = false;
        }

        terminal.draw(|f| render_ui(f, &mut app))?;

        if event::poll(Duration::from_millis(16))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Tab => toggle_focus(&mut app),
                    KeyCode::Esc => app.focus = Focus::CommandList,
                    KeyCode::Char('/') if matches!(app.focus, Focus::ManPage) => {
                        app.focus = Focus::Search;
                        app.search_query.clear();
                    }
                    _ => handle_key(&mut app, key.code).await,
                }
            }
        }

        // Ограничение FPS для экономии ресурсов
        let elapsed = now.elapsed();
        if elapsed < Duration::from_millis(16) {
            tokio::time::sleep(Duration::from_millis(16) - elapsed).await;
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn toggle_focus(app: &mut AppState) {
    app.focus = match app.focus {
        Focus::CommandList => Focus::ManPage,
        Focus::ManPage => Focus::CommandList,
        Focus::Search => Focus::ManPage,
    };
}

async fn handle_key(app: &mut AppState, key: KeyCode) {
    match app.focus {
        Focus::CommandList => handle_command_list_keys(app, key).await,
        Focus::ManPage => handle_man_page_keys(app, key),
        Focus::Search => handle_search_keys(app, key),
    }
}

async fn handle_command_list_keys(app: &mut AppState, key: KeyCode) {
    let commands_len = app.filtered_commands.len();
    if commands_len == 0 {
        return;
    }

    match key {
        KeyCode::Char(c) => {
            app.input.push(c);
            filter_commands(app);
            app.pending_man_load = true;
            app.last_input_time = Instant::now();
        }
        KeyCode::Backspace => {
            app.input.pop();
            filter_commands(app);
            app.pending_man_load = true;
            app.last_input_time = Instant::now();
        }
        KeyCode::Up => {
            if app.selected_idx > 0 {
                app.selected_idx -= 1;
                update_list_scroll(app);
                app.pending_man_load = true;
                app.last_input_time = Instant::now();
            }
        }
        KeyCode::Down => {
            if app.selected_idx < commands_len - 1 {
                app.selected_idx += 1;
                update_list_scroll(app);
                app.pending_man_load = true;
                app.last_input_time = Instant::now();
            }
        }
        KeyCode::PageUp => {
            app.selected_idx = app.selected_idx.saturating_sub(LIST_SIZE).max(0);
            update_list_scroll(app);
            app.pending_man_load = true;
            app.last_input_time = Instant::now();
        }
        KeyCode::PageDown => {
            app.selected_idx = (app.selected_idx + LIST_SIZE).min(commands_len - 1);
            update_list_scroll(app);
            app.pending_man_load = true;
            app.last_input_time = Instant::now();
        }
        KeyCode::Enter => {
            // Принудительная загрузка при нажатии Enter
            app.pending_man_load = true;
            load_current_man_page(app).await;
            app.pending_man_load = false;
        }
        _ => {}
    }
}

fn update_list_scroll(app: &mut AppState) {
    let visible_height = app.visible_list_range.1 - app.visible_list_range.0;

    if app.selected_idx < app.list_scroll {
        app.list_scroll = app.selected_idx;
    } else if app.selected_idx >= app.list_scroll + visible_height {
        app.list_scroll = app.selected_idx - visible_height + 1;
    }
}

fn filter_commands(app: &mut AppState) {
    let commands = app.man_db.get_commands();

    if app.input.is_empty() {
        app.filtered_commands = Arc::new(commands.clone());
    } else {
        let filtered: Vec<String> = commands
            .iter()
            .filter(|cmd| cmd.starts_with(&app.input))
            .cloned()
            .collect();
        app.filtered_commands = Arc::new(filtered);
    }
    app.selected_idx = 0;
    app.list_scroll = 0;
}

async fn load_current_man_page(app: &mut AppState) {
    if app.filtered_commands.is_empty() {
        app.man_content = Arc::new(vec!["No commands found".to_string()]);
        return;
    }

    let cmd = app.filtered_commands[app.selected_idx].clone();
    app.loading = true;
    let content = app.man_db.get_man_page(&cmd).await;
    app.man_content = content;
    app.loading = false;
    app.scroll = 0;
    update_search_matches(app);
}

fn handle_man_page_keys(app: &mut AppState, key: KeyCode) {
    match key {
        KeyCode::Up => app.scroll = app.scroll.saturating_sub(1),
        KeyCode::Down => app.scroll = app.scroll.saturating_add(1),
        KeyCode::PageUp => app.scroll = app.scroll.saturating_sub(PAGE_SIZE),
        KeyCode::PageDown => app.scroll = (app.scroll + PAGE_SIZE).min(app.man_content.len().saturating_sub(PAGE_SIZE)),
        KeyCode::Char('n') => next_search_match(app),
        KeyCode::Char('N') => prev_search_match(app),
        _ => {}
    }
}

fn handle_search_keys(app: &mut AppState, key: KeyCode) {
    match key {
        KeyCode::Enter => {
            update_search_matches(app);
            app.focus = Focus::ManPage;
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            update_search_matches(app);
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            update_search_matches(app);
        }
        KeyCode::Esc => {
            app.search_query.clear();
            app.search_matches = Arc::new(Vec::new());
            app.focus = Focus::ManPage;
        }
        _ => {}
    }
}

fn update_search_matches(app: &mut AppState) {
    let mut matches = Vec::new();

    if !app.search_query.is_empty() {
        for (i, line) in app.man_content.iter().enumerate() {
            if line.contains(&app.search_query) {
                matches.push(i);
            }
        }
    }

    app.search_matches = Arc::new(matches);
    app.current_match = 0;

    if !app.search_matches.is_empty() {
        app.scroll = app.search_matches[0].saturating_sub(PAGE_SIZE / 2);
    }
}

fn next_search_match(app: &mut AppState) {
    if app.search_matches.is_empty() {
        return;
    }

    app.current_match = (app.current_match + 1) % app.search_matches.len();
    let target_line = app.search_matches[app.current_match];
    app.scroll = target_line.saturating_sub(PAGE_SIZE / 2);
}

fn prev_search_match(app: &mut AppState) {
    if app.search_matches.is_empty() {
        return;
    }

    app.current_match = app.current_match.checked_sub(1)
        .unwrap_or(app.search_matches.len() - 1);

    let target_line = app.search_matches[app.current_match];
    app.scroll = target_line.saturating_sub(PAGE_SIZE / 2);
}

fn render_ui<B: tui::backend::Backend>(f: &mut tui::Frame<B>, app: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(10),
            ]
                .as_ref(),
        )
        .split(f.size());

    render_status_bar(f, app, chunks[0]);
    render_input(f, app, chunks[1]);
    render_main_content(f, app, chunks[2]);
}

fn render_status_bar<B: tui::backend::Backend>(f: &mut tui::Frame<B>, app: &AppState, area: Rect) {
    let status = if app.loading {
        "Loading..."
    } else {
        match app.focus {
            Focus::CommandList => "COMMAND LIST [Tab:Switch]",
            Focus::ManPage => "MAN PAGE [Tab:Switch /:Search n/N:Next/Prev]",
            Focus::Search => "SEARCH MODE [Enter:Apply Esc:Cancel]",
        }
    };

    let status_bar = Paragraph::new(status)
        .block(Block::default())
        .style(Style::default().bg(Color::DarkGray));

    f.render_widget(status_bar, area);
}

fn render_input<B: tui::backend::Backend>(f: &mut tui::Frame<B>, app: &AppState, area: Rect) {
    let input_text = match app.focus {
        Focus::CommandList | Focus::ManPage => format!("> {}", app.input),
        Focus::Search => format!("/{}", app.search_query),
    };

    let input = Paragraph::new(input_text.as_str())
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::Yellow));

    f.render_widget(input, area);
}

fn render_main_content<B: tui::backend::Backend>(f: &mut tui::Frame<B>, app: &mut AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(area);

    render_command_list(f, app, chunks[0]);
    render_man_page(f, app, chunks[1]);
}

fn render_command_list<B: tui::backend::Backend>(f: &mut tui::Frame<B>, app: &mut AppState, area: Rect) {
    let height = area.height as usize;
    app.visible_list_range = (app.list_scroll, app.list_scroll + height);

    let end = std::cmp::min(app.list_scroll + height, app.filtered_commands.len());
    let visible_commands = &app.filtered_commands[app.list_scroll..end];

    let items: Vec<ListItem> = visible_commands
        .iter()
        .map(|cmd| ListItem::new(cmd.as_str()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Commands"))
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ListState::default();
    state.select(Some(app.selected_idx - app.list_scroll));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_man_page<B: tui::backend::Backend>(f: &mut tui::Frame<B>, app: &AppState, area: Rect) {
    let height = area.height as usize;
    let start_line = app.scroll;
    let end_line = std::cmp::min(start_line + height, app.man_content.len());

    let visible_content: Vec<Spans> = app.man_content
        .iter()
        .enumerate()
        .skip(start_line)
        .take(end_line - start_line)
        .map(|(idx, line)| {
            let global_idx = idx + start_line;
            if app.search_matches.contains(&global_idx) {
                let search_index = app.search_matches.iter().position(|&i| i == global_idx).unwrap();
                let highlight = search_index == app.current_match;

                let mut spans = Vec::new();
                let mut remaining = line.as_str();

                while let Some(pos) = remaining.find(&app.search_query) {
                    let (before, after) = remaining.split_at(pos);
                    let (match_text, rest) = after.split_at(app.search_query.len());

                    spans.push(Span::raw(before));
                    spans.push(Span::styled(
                        match_text,
                        Style::default()
                            .bg(if highlight { Color::Red } else { Color::DarkGray })
                            .fg(if highlight { Color::White } else { Color::Black })
                    ));

                    remaining = rest;
                }
                spans.push(Span::raw(remaining));

                Spans::from(spans)
            } else {
                Spans::from(Span::raw(line.as_str()))
            }
        })
        .collect();

    let paragraph = Paragraph::new(visible_content)
        .block(Block::default().borders(Borders::ALL).title("Man Page"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}