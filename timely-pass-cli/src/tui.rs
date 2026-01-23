use anyhow::Result;
use arboard::Clipboard;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, path::PathBuf, time::{Duration, Instant}};
use timely_pass_sdk::store::{Credential, SecretStore};
use crate::commands::{prompt_passphrase, open_store_helper};

struct App {
    store: SecretStore,
    all_items: Vec<String>,
    filtered_items: Vec<String>,
    state: ListState,
    should_quit: bool,
    selected_cred: Option<Credential>,
    
    // Search
    search_query: String,
    is_searching: bool,
    
    // Secret
    show_secret: bool,
    clipboard: Option<Clipboard>,
    
    // Status
    status_message: Option<String>,
    status_time: Option<Instant>,
}

impl App {
    fn new(store: SecretStore) -> App {
        let mut all_items: Vec<String> = store.list_credentials().into_iter().map(|c| c.id.clone()).collect();
        all_items.sort();
        
        let filtered_items = all_items.clone();
        
        let mut state = ListState::default();
        if !filtered_items.is_empty() {
            state.select(Some(0));
        }
        
        let selected_cred = if !filtered_items.is_empty() {
            store.get_credential(&filtered_items[0]).cloned()
        } else {
            None
        };

        let clipboard = Clipboard::new().ok();

        App {
            store,
            all_items,
            filtered_items,
            state,
            should_quit: false,
            selected_cred,
            search_query: String::new(),
            is_searching: false,
            show_secret: false,
            clipboard,
            status_message: None,
            status_time: None,
        }
    }
    
    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
        self.status_time = Some(Instant::now());
    }
    
    pub fn update_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_items = self.all_items.clone();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered_items = self.all_items
                .iter()
                .filter(|id| id.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        
        // Reset selection
        if self.filtered_items.is_empty() {
            self.state.select(None);
            self.selected_cred = None;
        } else {
            self.state.select(Some(0));
            self.update_selection();
        }
    }
    
    pub fn on_down(&mut self) {
        if self.filtered_items.is_empty() { return; }
        
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.filtered_items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.update_selection();
    }

    pub fn on_up(&mut self) {
        if self.filtered_items.is_empty() { return; }

        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.filtered_items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.update_selection();
    }
    
    pub fn on_home(&mut self) {
        if !self.filtered_items.is_empty() {
            self.state.select(Some(0));
            self.update_selection();
        }
    }
    
    pub fn on_end(&mut self) {
        if !self.filtered_items.is_empty() {
            self.state.select(Some(self.filtered_items.len() - 1));
            self.update_selection();
        }
    }
    
    fn update_selection(&mut self) {
        if let Some(i) = self.state.selected() {
            if let Some(id) = self.filtered_items.get(i) {
                self.selected_cred = self.store.get_credential(id).cloned();
                self.show_secret = false; // Reset secret visibility on change
            }
        }
    }
    
    pub fn copy_secret(&mut self) {
        if let Some(cred) = &self.selected_cred {
            if let Some(cb) = &mut self.clipboard {
                // In a real app, we might need to handle non-utf8 data.
                // Assuming utf8 for now or hex encoding.
                let content = match String::from_utf8(cred.secret.data.clone()) {
                    Ok(s) => s,
                    Err(_) => hex::encode(&cred.secret.data),
                };
                
                if let Err(e) = cb.set_text(content) {
                    self.set_status(&format!("Clipboard error: {}", e));
                } else {
                    self.set_status("Secret copied to clipboard!");
                }
            } else {
                self.set_status("Clipboard not available");
            }
        }
    }
    
    pub fn toggle_secret(&mut self) {
        self.show_secret = !self.show_secret;
    }
}

pub async fn run(store_path: PathBuf) -> Result<()> {
    // 1. Initialize store (prompt for password first, outside TUI)
    println!("Initializing TUI...");
    let passphrase = prompt_passphrase(false)?;
    let store = open_store_helper(&store_path, &passphrase)?;

    // 2. Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. Create App
    let mut app = App::new(store);

    // 4. Run Loop
    let res = run_app(&mut terminal, &mut app);

    // 5. Restore Terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.is_searching {
                        match key.code {
                            KeyCode::Esc => {
                                app.is_searching = false;
                                app.search_query.clear();
                                app.update_filter();
                            }
                            KeyCode::Enter => {
                                app.is_searching = false;
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.update_filter();
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.update_filter();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                            KeyCode::Char('j') | KeyCode::Down => app.on_down(),
                            KeyCode::Char('k') | KeyCode::Up => app.on_up(),
                            KeyCode::Home => app.on_home(),
                            KeyCode::End => app.on_end(),
                            KeyCode::Char('/') => {
                                app.is_searching = true;
                                app.status_message = None;
                            }
                            KeyCode::Char('c') => app.copy_secret(),
                            KeyCode::Enter => app.toggle_secret(),
                            _ => {}
                        }
                    }
                }
            }
        }
        
        // Clear status message after 3 seconds
        if let Some(time) = app.status_time {
            if time.elapsed() > Duration::from_secs(3) {
                app.status_message = None;
                app.status_time = None;
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3), // Title
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(size);

    let title_text = if app.is_searching {
        format!("Search: {}_", app.search_query)
    } else {
        if !app.search_query.is_empty() {
             format!("Timely Pass (Filter: {})", app.search_query)
        } else {
             "Timely Pass - Secure Time-Based Store".to_string()
        }
    };

    let title_style = if app.is_searching {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    };

    let title = Paragraph::new(title_text)
        .style(title_style)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);
    
    // Main Content: Split into List and Details
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunks[1]);
        
    // Left: Credential List
    let items: Vec<ListItem> = app
        .filtered_items
        .iter()
        .map(|i| {
            ListItem::new(Line::from(vec![Span::raw(i)]))
        })
        .collect();
        
    let list_title = if app.filtered_items.is_empty() {
        "Credentials (Empty)"
    } else {
        "Credentials"
    };
        
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(list_title))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, main_chunks[0], &mut app.state);
    
    // Right: Details
    let detail_text = if let Some(cred) = &app.selected_cred {
        let created = cred.created_at.to_rfc3339();
        let updated = cred.updated_at.to_rfc3339();
        let type_str = format!("{:?}", cred.secret.type_);
        let policy_str = cred.policy_id.clone().unwrap_or_else(|| "None".to_string());
        let counter = cred.usage_counter;
        
        let secret_display = if app.show_secret {
            match String::from_utf8(cred.secret.data.clone()) {
                Ok(s) => s,
                Err(_) => format!("(Binary Data: {} bytes)", cred.secret.data.len()),
            }
        } else {
            "****************".to_string()
        };
        
        let secret_color = if app.show_secret { Color::Red } else { Color::DarkGray };
        
        vec![
            Line::from(vec![Span::styled("ID: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(&cred.id)]),
            Line::from(""),
            Line::from(vec![Span::styled("Type: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(type_str)]),
            Line::from(vec![Span::styled("Policy: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(policy_str)]),
            Line::from(vec![Span::styled("Usage Count: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(counter.to_string())]),
            Line::from(""),
            Line::from(vec![Span::styled("Created: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(created)]),
            Line::from(vec![Span::styled("Updated: ", Style::default().add_modifier(Modifier::BOLD)), Span::raw(updated)]),
            Line::from(""),
            Line::from(vec![Span::styled("Secret: ", Style::default().add_modifier(Modifier::BOLD)), Span::styled(secret_display, Style::default().fg(secret_color))]),
        ]
    } else {
        vec![Line::from("No credential selected")]
    };
    
    let detail = Paragraph::new(detail_text)
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: true });
    f.render_widget(detail, main_chunks[1]);
    
    // Footer
    let footer_text = if let Some(msg) = &app.status_message {
        format!("STATUS: {}", msg)
    } else {
        "q: Quit | /: Search | Enter: Reveal | c: Copy | \u{2191}\u{2193}: Nav".to_string()
    };
    
    let footer_style = if app.status_message.is_some() {
        Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    };
    
    let footer = Paragraph::new(footer_text)
        .style(footer_style);
    f.render_widget(footer, chunks[2]);
}
