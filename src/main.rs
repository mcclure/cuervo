// Text based web browser (experimental)
// Based on Ratatui popup example

use std::{error::Error, io};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    widgets::{Block, Clear, Paragraph, Wrap},
    Frame, Terminal,
};

use fluent::{FluentBundle, FluentValue, FluentResource, FluentArgs, FluentError};
use unic_langid::LanguageIdentifier;

use tui_input::backend::crossterm::EventHandler as InputEventHandler;
use tui_input::Input;

enum UiState { Base, Goto(Input) }

struct App {
    state: UiState,
    strings: FluentBundle<FluentResource>
}

impl App {
    const fn new(strings: FluentBundle<FluentResource>) -> Self {
        Self { state: UiState::Base, strings }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load strings
    let locale = sys_locale::get_locale().unwrap_or("en-US".to_owned());
    let langid: LanguageIdentifier = locale.parse().expect("Parsing failed");
    let strings = {
        let mut strings = FluentBundle::new(vec![langid.clone()]);
        let rawstring = match langid.language.as_str() {
            "es" => include_str!("strings/es.ftl"),
            "tok" => include_str!("strings/tok.ftl"),
            _ => include_str!("strings/en.ftl")
        };
        strings
            .add_resource(
                FluentResource::try_new(rawstring.to_string())
                    .expect("Failed to parse an FTL string.")
            ).expect("Failed to add FTL resources to the bundle.");
        strings
    };

    // create app and run it
    let app = App::new(strings);
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
                if let Event::Key(key @ KeyEvent { code, modifiers, .. }) = ev {
                    // Undocumented: CTRL-Q always quits
                    let ctrl = modifiers.intersects(KeyModifiers::CONTROL);
                    if code == KeyCode::Char('q') && ctrl {
                        return Ok(());
                    }
                    let accept = code == KeyCode::Enter;
                    let done = accept || 
                        // Undocumented: ESC and CTRL-C exit input
                        code == KeyCode::Esc || (code == KeyCode::Char('c') && ctrl);

                    if done {
                        app.state = UiState::Base;
                    } else {
                        input.handle_event(&Event::Key(key));
                    }
                }
            }
    }
}

fn naive_fluent(strings: &FluentBundle<FluentResource>, key:&str) -> String {
    let mut trash:Vec<FluentError> = Default::default();
    strings.format_pattern(
        strings.get_message(key).unwrap().value().unwrap(),
        None,
        &mut trash
    ).to_string()
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    let vertical = Layout::vertical([Constraint::Percentage(100)]);
    let [content] = vertical.areas(area);

    let intro = {
        let mut trash:Vec<FluentError> = Default::default();
        Paragraph::new(naive_fluent(&app.strings, "welcome"))
    }
        //.centered()
        .wrap(Wrap { trim: false });

    f.render_widget(intro, content);

    if let UiState::Goto(input) = &app.state {
        let block = Block::bordered().title(naive_fluent(&app.strings, "goto"));
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
