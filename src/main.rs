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
use servo::servo_url::ServoUrl;
use servo::webrender_traits::RenderingContext;
use servo_net::protocols::ProtocolRegistry;
use surfman::{Connection, Context, Device, SurfaceType};

use tui_input::backend::crossterm::EventHandler as InputEventHandler;
use tui_input::Input;

const VERSION:&str = "cuervo 0.1b"; // Not localized

enum UiState { Base, Goto(Input) }

enum BarState { None, UrlParse(String), UrlLoading }

#[cfg(feature = "debug_mode")]
const DEBUG_DISPLAY_FRESH:std::time::Duration = std::time::Duration::from_millis(100);

#[cfg(feature = "debug_mode")]
fn debug_display_reset() -> Option<std::time::Instant> { Some(std::time::Instant::now() + DEBUG_DISPLAY_FRESH) }

#[cfg(feature = "debug_mode")]
#[derive(Default)]
struct DebugMode {
    queue: std::collections::VecDeque<String>, // Messages to display
    flip: Option<std::time::Instant>, // Remaining ticks
}

struct App {
    state: UiState,
    bar_state: BarState,
    strings: FluentBundle<FluentResource>,
    browser_id: servo::TopLevelBrowsingContextId,
    servo: servo::Servo<glue::WindowCallbacks>,
    #[cfg(feature = "debug_mode")]
    debug_display: Option<DebugMode>, // If non-None do debug
}

impl App {
    const fn new(strings: FluentBundle<FluentResource>, browser_id: servo::TopLevelBrowsingContextId, servo: servo::Servo<glue::WindowCallbacks>) -> Self {
        Self {
            state: UiState::Base, bar_state:BarState::None, strings, browser_id, servo,

            #[cfg(feature = "debug_mode")]
            debug_display:None
        }
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

// INITIALIZE
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

        let user_agent = servo::default_user_agent_string_for(servo::UserAgent::Desktop);
        let mut cuervo_version_iter = VERSION.chars();
        let cuervo_version = cuervo_version_iter.next().unwrap().to_uppercase().collect::<String>()+cuervo_version_iter.as_str();

        let servo = servo::Servo::new(
            embed_handler,
            Rc::new(window),
            Some(format!("{user_agent} {cuervo_version} (like w3m)"), ), // User agent
            servo::compositing::CompositeTarget::Window,
        );

        App::new(strings, servo.browser_id, servo.servo)
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

// HANDLE EVENTS
fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    'run: loop {
        // Kick to draw
        terminal.draw(|f| ui(f, &app))?;

        let ev = event::read()?; // FIXME: No good, must pump events
        let mut sent_event = false;

        // Handle events
        match &mut app.state {
            UiState::Base =>
                if let Event::Key(key @ KeyEvent { code, modifiers, .. }) = ev {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            // Quit
                            KeyCode::Char('q') => break 'run,
                            // Go to
                            KeyCode::Char('g') => app.state = UiState::Goto("https://".into()),
                            
                            // Debug mode?!
                            #[cfg(feature = "debug_mode")]
                            KeyCode::Char('p') => if modifiers.contains(KeyModifiers::CONTROL) {
                                app.debug_display = if app.debug_display.is_none() {
                                    let mut d = DebugMode::default();
                                    d.flip = Some(std::time::Instant::now() + DEBUG_DISPLAY_FRESH*2);
                                    d.queue.push_back("Debug display entered (CTRL-P to revert)".to_string()); // Not localized
                                    Some(d)
                                } else { None };
                            },

                            _ => {}
                        }
                    }
                },
            UiState::Goto(input) =>
                if let Event::Key(key @ KeyEvent { code, modifiers, .. }) = ev {
                    // Undocumented: CTRL-Q always quits
                    let press = key.kind == KeyEventKind::Press;
                    let ctrl = modifiers.intersects(KeyModifiers::CONTROL);
                    if press && code == KeyCode::Char('q') && ctrl {
                        break 'run;
                    }
                    let accept = code == KeyCode::Enter;
                    let done = accept || 
                        // Undocumented: ESC and CTRL-C exit input
                        (press && (code == KeyCode::Esc || (code == KeyCode::Char('c') && ctrl)));

                    if done {
                        if accept {
                            // FIXME save the url // FIXME handle bad url // FIXME reuse views
                            let url = servo::servo_url::ServoUrl::parse(input.value()).expect("Not a real url");
                            sent_event = true;
                            app.servo.handle_events(vec![EmbedderEvent::NewWebView(url, app.browser_id)]);
                        }

                        app.state = UiState::Base;
                    } else {
                        input.handle_event(&Event::Key(key));
                    }
                }
        }

        // Rotate queue for debug display (if any)
        #[cfg(feature = "debug_mode")]
        if let Some(d) = &mut app.debug_display {
            if let Some(flip) = d.flip {
                if flip < std::time::Instant::now() {
                    d.queue.pop_front();
                    d.flip = if d.queue.is_empty() { None } else { debug_display_reset() }
                }
            }
        }

        // Pump servo queue
        if !sent_event {
            // TODO: Sleep 1ms?
            app.servo.handle_events(vec![]);

            for (_browser_id, event) in app.servo.get_events() {
                match event {
                    EmbedderMsg::LoadComplete => {
                        // TODO: Display lists I guess?
                    },
                    _=>()
                }

                #[cfg(feature = "debug_mode")] // Show every event in debug display
                if let Some(d) = &mut app.debug_display {
                    if d.flip.is_none() { d.flip = debug_display_reset(); }
                    d.queue.push_back(format!("{event:?}"));
                }
            }
        }
    }

    app.servo.handle_events(vec![EmbedderEvent::Quit]);

    'drain: loop {
        for (_browser_id, event) in app.servo.get_events() {
            //println!("{_browser_id:?}, {event:?}");
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

// DRAW
fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    let vertical = Layout::vertical([Constraint::Percentage(100)]);
    let [content] = vertical.areas(area);

    let intro = Paragraph::new(naive_fluent(&app.strings, "welcome"))
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

    #[cfg(feature = "debug_mode")]
    if let Some(d) = &app.debug_display {
        if let Some(text) = d.queue.front() {
            let text = format!("{text} ({})", d.queue.len());
            let bar = Paragraph::new(text.clone());
            let mut area = content;
            area.y = area.height-1;
            area.height=1;
            f.render_widget(bar, area);
        }
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
