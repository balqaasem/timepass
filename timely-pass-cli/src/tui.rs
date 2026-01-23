use anyhow::Result;
use arboard::Clipboard;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{io, path::PathBuf, time::{Duration, Instant}};
use timely_pass_sdk::store::{Credential, SecretStore, SecretType};
use timely_pass_sdk::crypto::generate_random_bytes;
use crate::commands::{prompt_passphrase, open_store_helper};
use chrono::Utc;

// --- States ---

enum AppMode {
    Normal,
    Search,
    Add(AddState),
    Rotate(RotateState),
    Delete(String), // ID to delete
}

struct AddState {
    id: String,
    secret: String,
    secret_type: SecretType,
    focus: AddFocus,
}

impl Default for AddState {
    fn default() -> Self {
        Self {
            id: String::new(),
            secret: String::new(),
            secret_type: SecretType::Password,
            focus: AddFocus::Id,
        }
    }
}

enum AddFocus {
    Id,
    Type,
    Secret,
}

struct RotateState {
    id: String,
    secret: String,
}

struct App {
    store: SecretStore,
    all_items: Vec<String>,
    filtered_items: Vec<String>,
    state: ListState,
    should_quit: bool,
    selected_cred: Option<Credential>,
    
    // Search
    search_query: String,
    
    // Modes
    mode: AppMode,
    
    // Secret Visibility
    show_secret: bool,
    clipboard: Option<Clipboard>,
    
    // Status
    status_message: Option<String>,
    status_time: Option<Instant>,
}

impl App {
    fn new(store: SecretStore) -> App {
        let mut app = App {
            store,
            all_items: Vec::new(),
            filtered_items: Vec::new(),
            state: ListState::default(),
            should_quit: false,
            selected_cred: None,
            search_query: String::new(),
            mode: AppMode::Normal,
            show_secret: false,
            clipboard: Clipboard::new().ok(),
            status_message: None,
            status_time: None,
        };
        app.refresh_list();
        app
    }
    
    fn refresh_list(&mut self) {
        let mut all_items: Vec<String> = self.store.list_credentials().into_iter().map(|c| c.id.clone()).collect();
        all_items.sort();
        self.all_items = all_items;
        self.update_filter();
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
        
        // Reset selection if invalid
        if self.filtered_items.is_empty() {
            self.state.select(None);
            self.selected_cred = None;
        } else {
            // Try to keep selection or select first
            if let Some(selected_idx) = self.state.selected() {
                 if selected_idx >= self.filtered_items.len() {
                      self.state.select(Some(0));
                 }
            } else {
                self.state.select(Some(0));
            }
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
                self.show_secret = false;
            }
        }
    }
    
    pub fn copy_secret(&mut self) {
        let (secret_data, cred_id) = if let Some(cred) = &self.selected_cred {
             (cred.secret.data.clone(), cred.id.clone())
        } else {
             return;
        };

        if let Some(cb) = &mut self.clipboard {
            let content = match String::from_utf8(secret_data.clone()) {
                Ok(s) => s,
                Err(_) => hex::encode(&secret_data),
            };
            
            if let Err(e) = cb.set_text(content) {
                self.set_status(&format!("Clipboard error: {}", e));
            } else {
                self.set_status("Secret copied to clipboard!");
                
                if let Err(e) = self.store.increment_usage(&cred_id) {
                     self.set_status(&format!("Error updating usage: {}", e));
                } else {
                     self.selected_cred = self.store.get_credential(&cred_id).cloned();
                }
            }
        } else {
            self.set_status("Clipboard not available");
        }
    }
    
    pub fn toggle_secret(&mut self) {
        self.show_secret = !self.show_secret;
        if self.show_secret {
             let cred_id = if let Some(cred) = &self.selected_cred {
                 Some(cred.id.clone())
             } else {
                 None
             };

             if let Some(id) = cred_id {
                 if let Err(e) = self.store.increment_usage(&id) {
                      self.set_status(&format!("Error updating usage: {}", e));
                 } else {
                     self.selected_cred = self.store.get_credential(&id).cloned();
                 }
             }
        }
    }

    // --- Actions ---

    fn delete_current(&mut self) {
        if let Some(cred) = &self.selected_cred {
            self.mode = AppMode::Delete(cred.id.clone());
        }
    }

    fn confirm_delete(&mut self) {
        if let AppMode::Delete(id) = &self.mode {
            if let Err(e) = self.store.remove_credential(id) {
                self.set_status(&format!("Error removing credential: {}", e));
            } else {
                self.set_status(&format!("Credential '{}' removed.", id));
                self.refresh_list();
            }
        }
        self.mode = AppMode::Normal;
    }

    fn start_add(&mut self) {
        self.mode = AppMode::Add(AddState::default());
    }

    fn confirm_add(&mut self) {
        if let AppMode::Add(state) = &self.mode {
            if state.id.is_empty() {
                self.set_status("ID cannot be empty");
                return;
            }
            
            let secret_bytes = if state.secret.is_empty() {
                generate_random_bytes(32)
            } else {
                state.secret.as_bytes().to_vec()
            };

            let cred = Credential::new(state.id.clone(), state.secret_type.clone(), secret_bytes);
            if let Err(e) = self.store.add_credential(cred) {
                self.set_status(&format!("Error adding credential: {}", e));
            } else {
                self.set_status(&format!("Credential '{}' added.", state.id));
                self.refresh_list();
                self.mode = AppMode::Normal;
            }
        }
    }
    
    fn start_rotate(&mut self) {
        if let Some(cred) = &self.selected_cred {
            self.mode = AppMode::Rotate(RotateState {
                id: cred.id.clone(),
                secret: String::new(),
            });
        }
    }

    fn confirm_rotate(&mut self) {
        if let AppMode::Rotate(state) = &self.mode {
            let new_secret_bytes = if state.secret.is_empty() {
                generate_random_bytes(32)
            } else {
                state.secret.as_bytes().to_vec()
            };

            if let Some(mut cred) = self.store.get_credential(&state.id).cloned() {
                cred.secret.data = new_secret_bytes;
                cred.updated_at = Utc::now();
                
                if let Err(e) = self.store.add_credential(cred) {
                     self.set_status(&format!("Error rotating credential: {}", e));
                } else {
                     self.set_status(&format!("Credential '{}' rotated.", state.id));
                     self.refresh_list();
                }
            } else {
                self.set_status("Credential not found during rotate");
            }
        }
        self.mode = AppMode::Normal;
    }
}

pub async fn run(store_path: PathBuf) -> Result<()> {
    println!("Initializing TUI...");
    let passphrase = prompt_passphrase(false)?;
    let store = open_store_helper(&store_path, &passphrase)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(store);
    let res = run_app(&mut terminal, &mut app);

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
                    match &mut app.mode {
                        AppMode::Normal => {
                             match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                                KeyCode::Char('j') | KeyCode::Down => app.on_down(),
                                KeyCode::Char('k') | KeyCode::Up => app.on_up(),
                                KeyCode::Home => app.on_home(),
                                KeyCode::End => app.on_end(),
                                KeyCode::Char('/') => {
                                    app.mode = AppMode::Search;
                                    app.status_message = None;
                                }
                                KeyCode::Char('c') => app.copy_secret(),
                                KeyCode::Enter => app.toggle_secret(),
                                KeyCode::Char('a') => app.start_add(),
                                KeyCode::Char('d') | KeyCode::Delete => app.delete_current(),
                                KeyCode::Char('r') => app.start_rotate(),
                                _ => {}
                            }
                        },
                        AppMode::Search => {
                            match key.code {
                                KeyCode::Esc => {
                                    app.mode = AppMode::Normal;
                                    app.search_query.clear();
                                    app.update_filter();
                                }
                                KeyCode::Enter => {
                                    app.mode = AppMode::Normal;
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
                        },
                        AppMode::Delete(_) => {
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Enter => app.confirm_delete(),
                                KeyCode::Char('n') | KeyCode::Esc => app.mode = AppMode::Normal,
                                _ => {}
                            }
                        },
                        AppMode::Add(state) => {
                             match key.code {
                                KeyCode::Esc => app.mode = AppMode::Normal,
                                KeyCode::Tab => {
                                    state.focus = match state.focus {
                                        AddFocus::Id => AddFocus::Type,
                                        AddFocus::Type => AddFocus::Secret,
                                        AddFocus::Secret => AddFocus::Id,
                                    }
                                },
                                KeyCode::Enter => {
                                    app.confirm_add();
                                },
                                KeyCode::Backspace => {
                                    match state.focus {
                                        AddFocus::Id => { state.id.pop(); },
                                        AddFocus::Secret => { state.secret.pop(); },
                                        _ => {}
                                    }
                                },
                                KeyCode::Left | KeyCode::Right => {
                                    if let AddFocus::Type = state.focus {
                                        state.secret_type = match state.secret_type {
                                            SecretType::Password => SecretType::Key,
                                            SecretType::Key => SecretType::Token,
                                            SecretType::Token => SecretType::Password,
                                        };
                                    }
                                },
                                KeyCode::Char(c) => {
                                    match state.focus {
                                        AddFocus::Id => state.id.push(c),
                                        AddFocus::Secret => state.secret.push(c),
                                        _ => {}
                                    }
                                }
                                _ => {}
                             }
                        },
                        AppMode::Rotate(state) => {
                            match key.code {
                                KeyCode::Esc => app.mode = AppMode::Normal,
                                KeyCode::Enter => app.confirm_rotate(),
                                KeyCode::Backspace => { state.secret.pop(); },
                                KeyCode::Char(c) => state.secret.push(c),
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        
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
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(size);

    let title_text = match &app.mode {
        AppMode::Search => format!("Search: {}_", app.search_query),
        AppMode::Add(_) => "Adding New Credential".to_string(),
        AppMode::Delete(_) => "Confirm Deletion".to_string(),
        AppMode::Rotate(_) => "Rotating Credential".to_string(),
        AppMode::Normal => {
             if !app.search_query.is_empty() {
                 format!("Timely Pass (Filter: {})", app.search_query)
             } else {
                 "Timely Pass - Secure Time-Based Store".to_string()
             }
        }
    };

    let title_style = match app.mode {
        AppMode::Normal => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    };

    let title = Paragraph::new(title_text)
        .style(title_style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);
    
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
        .split(chunks[1]);
        
    let items: Vec<ListItem> = app
        .filtered_items
        .iter()
        .map(|i| {
            ListItem::new(Line::from(vec![Span::raw(i)]))
        })
        .collect();
        
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Credentials"))
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");
    f.render_stateful_widget(list, main_chunks[0], &mut app.state);
    
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
    
    let footer_text = if let Some(msg) = &app.status_message {
        format!("STATUS: {}", msg)
    } else {
        match app.mode {
             AppMode::Normal => "q: Quit | a: Add | d: Delete | r: Rotate | /: Search | Enter: Reveal | c: Copy".to_string(),
             AppMode::Search => "Esc: Cancel | Enter: Done".to_string(),
             AppMode::Delete(_) => "y: Confirm Delete | n/Esc: Cancel".to_string(),
             AppMode::Add(_) => "Tab: Next Field | Enter: Save | Esc: Cancel | \u{2190}\u{2192}: Cycle Type".to_string(),
             AppMode::Rotate(_) => "Enter: Save | Esc: Cancel | (Leave empty to generate)".to_string(),
        }
    };
    
    let footer_style = if app.status_message.is_some() {
        Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    };
    
    let footer = Paragraph::new(footer_text)
        .style(footer_style);
    f.render_widget(footer, chunks[2]);

    // --- Popups ---

    match &app.mode {
        AppMode::Delete(id) => {
             let block = Block::default().title("Confirm Delete").borders(Borders::ALL);
             let area = centered_rect(60, 20, size);
             f.render_widget(Clear, area); // Clear background
             f.render_widget(block, area);
             
             let text = Paragraph::new(format!("Are you sure you want to delete '{}'?\n\n(y) Yes   (n) No", id))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
                
             let inner_area = centered_rect(50, 10, size); // rough approximation for inner content
             f.render_widget(text, inner_area);
        },
        AppMode::Add(state) => {
             let block = Block::default().title("Add Credential").borders(Borders::ALL);
             let area = centered_rect(60, 40, size);
             f.render_widget(Clear, area);
             f.render_widget(block, area);
             
             let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(3), // ID
                    Constraint::Length(3), // Type
                    Constraint::Length(3), // Secret
                    Constraint::Min(0)
                ].as_ref())
                .split(area);
                
             let id_style = if let AddFocus::Id = state.focus { Style::default().fg(Color::Yellow) } else { Style::default() };
             let type_style = if let AddFocus::Type = state.focus { Style::default().fg(Color::Yellow) } else { Style::default() };
             let secret_style = if let AddFocus::Secret = state.focus { Style::default().fg(Color::Yellow) } else { Style::default() };
             
             let id_p = Paragraph::new(state.id.as_str()).block(Block::default().borders(Borders::ALL).title("ID")).style(id_style);
             let type_p = Paragraph::new(format!("{:?}", state.secret_type)).block(Block::default().borders(Borders::ALL).title("Type (<- ->)")).style(type_style);
             let secret_p = Paragraph::new(state.secret.as_str()).block(Block::default().borders(Borders::ALL).title("Secret (Empty=Auto)")).style(secret_style);
             
             f.render_widget(id_p, layout[0]);
             f.render_widget(type_p, layout[1]);
             f.render_widget(secret_p, layout[2]);
        },
        AppMode::Rotate(state) => {
             let block = Block::default().title("Rotate Credential").borders(Borders::ALL);
             let area = centered_rect(60, 20, size);
             f.render_widget(Clear, area);
             f.render_widget(block, area);
             
             let layout = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([
                    Constraint::Length(3), // Secret
                    Constraint::Min(0)
                ].as_ref())
                .split(area);
                
             let secret_p = Paragraph::new(state.secret.as_str())
                .block(Block::default().borders(Borders::ALL).title("New Secret (Empty=Auto)"))
                .style(Style::default().fg(Color::Yellow));
             
             f.render_widget(secret_p, layout[0]);
        },
        _ => {}
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1]);

    layout[1]
}
