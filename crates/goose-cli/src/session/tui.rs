use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::{
    io::{self, Stdout},
    time::Duration,
};
use mcp_core::role::Role;

/// Very small abstraction layer so we don't have to expose the whole `ratatui` types to the parent
/// modules. The struct just keeps the terminal alive while the TUI runs.
pub struct GooseTui<'a> {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    /// Buffer holding the user input while they type
    input: String,
    /// Shared reference to an interactive [`crate::Session`]. Held as mutable reference so we can
    /// push messages and request completions.
    session: &'a mut crate::Session,
    /// Scroll offset for the chat history panel
    scroll: u16,
    /// Stores the rendered text for each historical message. We keep things as simple `String`s for
    /// now – every line break yields a new line on screen which is good enough for a first cut.
    history: Vec<(String, bool /* is_user */)>,
}

impl<'a> GooseTui<'a> {
    pub fn new(session: &'a mut crate::Session) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            input: String::new(),
            session,
            scroll: 0,
            history: Vec::new(),
        })
    }

    /// Consumes the TUI, restoring the terminal.
    fn teardown(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    /// Run the TUI main loop. This will block until the user presses <Esc>.
    pub async fn run(mut self) -> Result<()> {
        loop {
            // Draw UI
            self.terminal.draw(|f| {
                let size = f.size();

                // Split screen into message area + input line (3 rows)
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                    .split(size);

                // Render message history
                let history_lines: Vec<Line> = self
                    .history
                    .iter()
                    .flat_map(|(line, is_user)| {
                        let clr = if *is_user { Color::Yellow } else { Color::White };
                        line.split('\n')
                            .map(move |l| {
                                Line::from(vec![Span::styled(
                                    l.to_owned(),
                                    Style::default().fg(clr),
                                )])
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect();

                let history_para = Paragraph::new(history_lines)
                    .block(Block::default().title("Messages").borders(Borders::ALL))
                    .wrap(Wrap { trim: false });
                f.render_widget(history_para, chunks[0]);

                // Render input area
                let input_para = Paragraph::new(self.input.as_str())
                    .style(Style::default().fg(Color::Cyan))
                    .block(Block::default().title("Input (Esc to quit)").borders(Borders::ALL));
                f.render_widget(input_para, chunks[1]);
                // Put cursor at end of input buffer
                let x = chunks[1].x + (self.input.len() as u16) + 1;
                let y = chunks[1].y + 1;
                #[allow(deprecated)]
                {
                    f.set_cursor(x, y);
                }
            })?;

            // Handle events
            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char(c) => {
                            self.input.push(c);
                        }
                        KeyCode::Backspace => {
                            self.input.pop();
                        }
                        KeyCode::Enter => {
                            let user_msg = self.input.trim().to_string();
                            if !user_msg.is_empty() {
                                // Push to local history first so the user gets immediate feedback
                                self.history.push((format!("You: {}", &user_msg), true));

                                // Clear input buffer before awaiting async call so the UI remains responsive
                                self.input.clear();

                                // Run the agent interaction synchronously for now (will freeze UI briefly).
                                if let Err(e) = self.session.process_message(user_msg).await {
                                    self.history.push((format!("Error: {}", e), false));
                                }

                                // After processing (successful or not), refresh from session's message history.
                                let new_msgs = self.session.message_history();
                                self.history = new_msgs
                                    .iter()
                                    .flat_map(|m| {
                                        let mut lines = Vec::new();
                                        let sender = match m.role {
                                            Role::User => "You",
                                            Role::Assistant => "Assistant",
                                        };
                                        let text_concat = m.as_concat_text();
                                        for l in text_concat.split('\n') {
                                            let is_user = matches!(m.role, Role::User);
                                            lines.push((format!("{}: {}", sender, l), is_user));
                                        }
                                        lines
                                    })
                                    .collect();
                            }
                        }
                        KeyCode::Esc => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        self.teardown()
    }
} 