//! # [Ratatui] Popup example
//!
//! The latest version of this example is available in the [examples] folder in the repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui-org/ratatui
//! [examples]: https://github.com/ratatui-org/ratatui/blob/main/examples
//! [examples readme]: https://github.com/ratatui-org/ratatui/blob/main/examples/README.md

// See also https://github.com/joshka/tui-popup and
// https://github.com/sephiroth74/tui-confirm-dialog

use std::{error::Error, io};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    widgets::{Block, Clear, Paragraph, Wrap},
    Frame, Terminal,
};

use tui_input::backend::crossterm as input_backend;
use tui_input::backend::crossterm::EventHandler as InputEventHandler;
use tui_input::Input;

const APPNAME:&str = "cuervo";

enum UiState { Base, Goto(Input) }

struct App {
    state: UiState,
}

impl App {
    const fn new() -> Self {
        Self { state: UiState::Base, }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{err:?}");
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &app))?;

        let ev = event::read()?;

        match &mut app.state {
            UiState::Base =>
                if let Event::Key(key) = ev {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            // Quit
                            KeyCode::Char('q') => return Ok(()),
                            // Go to
                            KeyCode::Char('g') => app.state = UiState::Goto("https://".into()),
                            _ => {}
                        }
                    }
                },
            UiState::Goto(input) =>
                if let Event::Key(key @ KeyEvent { code, .. }) = ev {
                    match code {
                        KeyCode::Esc | KeyCode::Enter => {
                            app.state = UiState::Base;
                        }
                        _ => {
                            input.handle_event(&Event::Key(key));
                        }
                    }
                }
            }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    let vertical = Layout::vertical([Constraint::Percentage(100)]);
    let [content] = vertical.areas(area);

    let text = format!("\nWelcome to {APPNAME}.\n\
                        \n\
                        Controls:\n\
                        \x20\x20\x20\x20g: Go to URL.\n\
                        \x20\x20\x20\x20q: Quit.");

    let intro = Paragraph::new(text)
        //.centered()
        .wrap(Wrap { trim: false });

    f.render_widget(intro, content);

    if let UiState::Goto(input) = &app.state {
        let block = Block::bordered().title("Go to URL");
        let area = centered_rect(60, 20, area);
        let area = Rect {height:3, ..area}; // Dont actually want relative height

        let inner = block.inner(area);
        let width = area.width.max(1) - 1;
        let scroll_amount = input.visual_scroll(width as usize);
        let input_widget = Paragraph::new(input.value())
            .style(ratatui::style::Style::default())
            .scroll((0, scroll_amount as u16));

        f.render_widget(Clear, area); //this clears out the background
        f.render_widget(block, area);

        f.render_widget(input_widget, inner); //this clears out the background

        f.set_cursor_position((
                // Put cursor past the end of the input text
                inner.x
                    + ((input.visual_cursor()).max(scroll_amount) - scroll_amount) as u16
                    + 0,
                // Move one line down, from the border to the input line
                inner.y,
            ))
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
