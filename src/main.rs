// Text based web browser (experimental)
// Based on Ratatui popup example

mod glue;

use std::{error::Error, io};
use std::cell::RefCell;
use std::rc::Rc;

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

use servo::embedder_traits::{EventLoopWaker, EmbedderMsg, EmbedderProxy};
use servo::compositing::windowing::{EmbedderEvent, EmbedderMethods};
use servo::webrender_traits::RenderingContext;
use servo_net::protocols::ProtocolRegistry;
use surfman::{Connection, Context, Device, SurfaceType};

use tui_input::backend::crossterm::EventHandler as InputEventHandler;
use tui_input::Input;

const VERSION:&str = "cuervo 0.1b"; // Not localized

enum UiState { Base, Goto(Input) }

struct App {
    state: UiState,
    strings: FluentBundle<FluentResource>,
    servo: servo::Servo<glue::WindowCallbacks>,
}

impl App {
    const fn new(strings: FluentBundle<FluentResource>, servo: servo::Servo<glue::WindowCallbacks>) -> Self {
        Self { state: UiState::Base, strings, servo }
    }
}

// Handle event loop messages
struct Waker { // TODO
}

impl EventLoopWaker for Waker {
    // Required methods
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Waker {})
    }
    fn wake(&self) {
    }
}

// Handle messages from glue.rs
struct HostHandler {
}

impl glue::HostTrait for HostHandler {
    fn on_animating_changed(&self, _animating: bool) {
    }
}

struct EmbedHandler {
    event_loop_waker: Box<dyn EventLoopWaker>,
}

impl EmbedHandler {
    pub fn new(event_loop_waker: Box<dyn EventLoopWaker>) -> EmbedHandler {
        EmbedHandler { event_loop_waker }
    }
}

impl EmbedderMethods for EmbedHandler {
    fn create_event_loop_waker(&mut self) -> Box<dyn EventLoopWaker> {
        self.event_loop_waker.clone()
    }

    fn register_webxr(&mut self, _xr: &mut servo_webxr::MainThreadRegistry,
        _embedder_proxy: EmbedderProxy,
    ) {
        // XR support not planned
    }

    fn get_protocol_handlers(&self) -> ProtocolRegistry {
        let mut registry = ProtocolRegistry::default();
        // TODO support 
//        registry.register("servo", servo_handler::ServoProtocolHander::default());
        registry
    }

    fn get_version_string(&self) -> Option<String> {
        Some(VERSION.into())
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
    let app = {
        let waker = Box::new(Waker{});
        let embed_handler = Box::new(EmbedHandler::new(waker));
        let size = terminal.size().unwrap();

        let connection = Connection::new().expect("Failed to create connection");
        let adapter = connection
            .create_software_adapter()
            .expect("Failed to create adapter");

        // FIXME A rendering context is required, but why?
        let surface_type = SurfaceType::Generic { size: euclid::Size2D::new(1 as i32, 1 as i32) };
        let rendering_context = RenderingContext::create(&connection, &adapter, surface_type)
            .expect("Failed to create WR surfman");

        let window = glue::WindowCallbacks::new(
            Box::new(HostHandler {}),
            RefCell::new(glue::Coordinates::new(0, 0, size.width as i32, size.height as i32, 1, 1)), // TODO update on resize // FIXME 1x1 framebuffer?
            1.0/20.0, // TODO pick number less arbitrarily
            rendering_context
        );

        let servo = servo::Servo::new(
            embed_handler,
            Rc::new(window),
            Some("desktop".into()),
            servo::compositing::CompositeTarget::Window,
        );

        App::new(strings, servo.servo)
    };
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
    'run: loop {
        terminal.draw(|f| ui(f, &app))?;

        let ev = event::read()?;

        match &mut app.state {
            UiState::Base =>
                if let Event::Key(key) = ev {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            // Quit
                            KeyCode::Char('q') => break 'run,
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
                        break 'run;
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


    app.servo.handle_events(vec![EmbedderEvent::Quit]);

    'drain: loop {
        for (browser_id, event) in app.servo.get_events() {
            //println!("{browser_id:?}, {event:?}");
            if let EmbedderMsg::Shutdown = event {
                break 'drain;
            }
        }

        // TODO: Sleep 1ms
        app.servo.handle_events(vec![]);
    }
    app.servo.deinit();

    Ok(())
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
