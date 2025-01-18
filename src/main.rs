// Text based web browser (experimental)
// Based on Ratatui popup example and servo/ports/servoshell

mod glue;

use std::{error::Error, io};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use futures::StreamExt;

use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    widgets::{Block, Clear, Paragraph, Wrap},
    Frame, Terminal,
};

use fluent::{FluentBundle, FluentValue, FluentResource, FluentArgs, FluentError};
use unic_langid::LanguageIdentifier;

use servo::base::id::WebViewId;
use servo::config::{opts, prefs::Preferences};
use servo::compositing::windowing::{EmbedderEvent, EmbedderMethods};
use servo::embedder_traits::{EventLoopWaker, EmbedderMsg, EmbedderProxy};
use servo::servo_url::ServoUrl;
use servo::webrender_traits::RenderingContext;
use servo_net::protocols::ProtocolRegistry;
use surfman::{Connection, Context, Device, SurfaceType};

use tui_input::backend::crossterm::EventHandler as InputEventHandler;
use tui_input::Input;

const VERSION:&str = "cuervo 0.1b"; // Not localized

enum UiState { Base, Goto(Input) }

enum BarState { None, UrlParse(String), UrlLoading }

// FIXME: Overall, debug_mode feels a little heavyweight for a debug feature
#[cfg(feature = "debug_mode")]
const DEBUG_DISPLAY_FRESH:std::time::Duration = std::time::Duration::from_millis(400);

#[cfg(feature = "debug_mode")]
#[derive(Default)]
struct DebugMode {
    queue: std::collections::VecDeque<String>, // Messages to display
    flip: Option<std::time::Instant>, // Remaining ticks
}

#[cfg(feature = "debug_mode")]
impl DebugMode {
    fn reset(&mut self, timer: &mut async_io::Timer, multiplier:u32) { // 0 for "never"
        if 0 == multiplier {
            self.flip = None;
            *timer = async_io::Timer::never();
        } else {
            let time = std::time::Instant::now() + DEBUG_DISPLAY_FRESH*multiplier;
            self.flip = Some(time);
            *timer = async_io::Timer::at(time);
        }
    }
}

enum StatusStyle {
    Info,
    Error
}

struct App {
    state: UiState,
    bar_state: BarState,
    should_quit: bool,
    strings: FluentBundle<FluentResource>,
    browser_id: servo::TopLevelBrowsingContextId,
    servo_wakeup: Arc<tokio::sync::Notify>,
    servo: servo::Servo<glue::WindowCallbacks>,
    reset_page_text: bool,
    page_scroll:isize,
    page_display: Option<String>,
    status_display: Option<(StatusStyle, String)>,

    #[cfg(feature = "debug_mode")]
    debug_display: Option<DebugMode>, // If non-None do debug
}

impl App {
    const fn new(strings: FluentBundle<FluentResource>, browser_id: servo::TopLevelBrowsingContextId, servo_wakeup: Arc<tokio::sync::Notify>, servo: servo::Servo<glue::WindowCallbacks>) -> Self {
        Self {
            state: UiState::Base, bar_state:BarState::None, should_quit:false, strings, browser_id, servo_wakeup, servo, reset_page_text:true, page_scroll:0, page_display:None, status_display:None,

            #[cfg(feature = "debug_mode")]
            debug_display:None
        }
    }
}

// Handle event loop messages
struct Waker { // TODO
    wakeup_send: Arc<tokio::sync::Notify>
}

impl EventLoopWaker for Waker {
    // Required methods
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(Waker {wakeup_send: self.wakeup_send.clone()})
    }
    fn wake(&self) {
        self.wakeup_send.notify_one();
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

    // fn register_webxr(&mut self, _xr: &mut servo_webxr::MainThreadRegistry,
    //     _embedder_proxy: EmbedderProxy,
    // ) {
    //     // XR support not planned
    // }

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
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // rustls crashes if we don't do this early (how early? could it go after UI draw?)
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Error initializing crypto provider");

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
        let servo_wakeup = Arc::new(tokio::sync::Notify::new());
        let waker = Box::new(Waker{wakeup_send:servo_wakeup.clone()});
        let embed_handler = Box::new(EmbedHandler::new(waker));
        let size = terminal.size().unwrap();

        let connection = Connection::new().expect("Failed to create connection");
        let adapter = connection
            .create_software_adapter()
            .expect("Failed to create adapter");

        // FIXME A rendering context is required, but why?
        let rendering_context = RenderingContext::create(&connection, &adapter, Some(euclid::Size2D::new(1 as i32, 1 as i32)))
            .expect("Failed to create WR surfman");

        let window = glue::WindowCallbacks::new(
            Box::new(HostHandler {}),
            RefCell::new(glue::Coordinates::new(0, 0, size.width as i32, size.height as i32, 1, 1)), // TODO update on resize // FIXME 1x1 framebuffer?
            1.0/20.0, // TODO pick number less arbitrarily
        );

        let user_agent = servo::default_user_agent_string_for(servo::UserAgent::Desktop);
        let mut cuervo_version_iter = VERSION.chars();
        let cuervo_version = cuervo_version_iter.next().unwrap().to_uppercase().collect::<String>()+cuervo_version_iter.as_str();

        let servo = servo::Servo::new(
            opts::default_opts(),
            Preferences::default(),
            rendering_context,
            embed_handler,
            Rc::new(window),
            Some(format!("{user_agent} {cuervo_version} (like w3m)"), ), // User agent
            servo::compositing::CompositeTarget::Window,
        );

        App::new(strings, WebViewId::new(), servo_wakeup, servo)
    };
    let res = run_app(&mut terminal, app).await;

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
async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let mut events = crossterm::event::EventStream::new();
    app.should_quit = false;

    let mut debug_display_timer = async_io::Timer::never();

    loop {
        // Kick to draw
        terminal.draw(|f| ui(f, &app))?;

        if app.should_quit { break; }

        tokio::select! {
            Some(Ok(ev)) = events.next() => {
                // Handle events
                match &mut app.state {
                    UiState::Base =>
                        if let Event::Key(key @ KeyEvent { code, modifiers, .. }) = ev {
                            let ctrl = modifiers.intersects(KeyModifiers::CONTROL);

                            if key.kind == KeyEventKind::Press {
                                match key.code {
                                    // Quit
                                    KeyCode::Char('q') => app.should_quit = true,
                                    // Go to
                                    KeyCode::Char('g') => app.state = UiState::Goto("https://".into()),

                                    KeyCode::Down | KeyCode::Char('j') => app.page_scroll += 1,
                                    KeyCode::Up | KeyCode::Char('k') => app.page_scroll = (app.page_scroll - 1).max(0),

                                    KeyCode::Char('f') | KeyCode::Char('b') => 
                                        if ctrl {
                                            let jump = if let Ok(ratatui::prelude::Size{height,..}) = terminal.size() {
                                                (height as isize-4).max(1)
                                            } else {
                                                8
                                            };
                                            let jump = jump * if key.code == KeyCode::Char('f') { 1 } else { -1 };
                                            app.page_scroll = (app.page_scroll + jump).max(1);
                                        },

                                    // Undocumented: Esc or CTRL-C to clear errors
                                    // TODO: Also clear on scroll
                                    KeyCode::Esc | KeyCode::Char('c') => {
                                        if key.code != KeyCode::Char('c') || ctrl {
                                            if let Some((StatusStyle::Error, _)) = app.status_display {
                                                app.status_display = None;
                                            }
                                        }
                                    }

                                    // Debug mode?!
                                    #[cfg(feature = "debug_mode")]
                                    KeyCode::Char('p') => if modifiers.contains(KeyModifiers::CONTROL) {
                                        app.debug_display = if app.debug_display.is_none() {
                                            let mut d = DebugMode::default();
                                            d.reset(&mut debug_display_timer, 2);
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
                                app.should_quit = true;
                            } else {
                                let accept = code == KeyCode::Enter;
                                let done = accept ||
                                    // Undocumented: ESC and CTRL-C exit input
                                    (press && (code == KeyCode::Esc || (code == KeyCode::Char('c') && ctrl)));

                                if done {
                                    if accept {
                                        // FIXME save the url // FIXME handle bad url // FIXME reuse views
                                        let url = servo::servo_url::ServoUrl::parse(input.value());

                                        if let Ok(url) = url {
                                            app.reset_page_text = true;
                                            app.servo.handle_events(vec![EmbedderEvent::NewWebView(url.clone(), app.browser_id)]);

                                            let mut args = FluentArgs::new();
                                            if let Some(host) = url.host() {
                                                args.set("url_slug", host.to_string());
                                            } else {
                                                args.set("url_slug", "(unknown)");
                                            }
                                            app.status_display = Some((StatusStyle::Info, naive_fluent_args(&app.strings, "loading", args)))
                                        } else {
                                            app.status_display = Some((StatusStyle::Error, naive_fluent(&app.strings, "bad_url")))
                                        }
                                    }

                                    app.state = UiState::Base;
                                } else {
                                    input.handle_event(&Event::Key(key));
                                }
                            }
                        }
                }
            },
            _ = app.servo_wakeup.notified() => {}, // Don't do anything, just wake up
            _ = &mut debug_display_timer => {},
        }

        if !app.should_quit {

            // Rotate queue for debug display (if any)
            #[cfg(feature = "debug_mode")]
            if let Some(d) = &mut app.debug_display {
                if let Some(flip) = d.flip {
                    if flip < std::time::Instant::now() {
                        d.queue.pop_front();
                        d.reset(&mut debug_display_timer, if d.queue.is_empty() { 0 } else { 1 });
                    }
                }
            }

            // Pump servo queue
            app.servo.handle_events(vec![]);

            for (_browser_id, event) in app.servo.get_events() {
                match &event {
                    EmbedderMsg::CuervoReportStrings(v) => {
                        let mut page_text:String = Default::default();
                        for s in v {
                            if !s.is_empty() && !s.trim().is_empty() {
                                page_text += s.trim_end();
                                page_text += "\n";
                            }
                        }
                        if app.reset_page_text || app.page_display.is_none() { // Second clause should be impossible
                            if page_text.is_empty() {
                                page_text = naive_fluent(&app.strings, "empty_page");
                            } else {
                                app.reset_page_text = false;
                            }
                            app.page_display = Some(page_text);
                        } else { // Some sites get multiple passes
                            if !page_text.is_empty() {
                                app.page_display = Some(app.page_display.unwrap() + &page_text);
                            }
                        }
                    },
                    EmbedderMsg::ReadyToPresent(_) => { // FIXME: Check IDs?
                        app.status_display = None;
                    },
                    _=>()
                }

                #[cfg(feature = "debug_mode")] // Show every event in debug display
                if let Some(d) = &mut app.debug_display {
                    if d.flip.is_none() { d.reset(&mut debug_display_timer, 2); }
                    d.queue.push_back(format!("{event:?}"));
                }
            }
        }
    }

    // Must shut down servo thread before quit or it crashes
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
    ).to_string() // FIXME: Consider panic if trash full
}

fn naive_fluent_args(strings: &FluentBundle<FluentResource>, key:&str, args:FluentArgs) -> String {
    let mut trash:Vec<FluentError> = Default::default();
    strings.format_pattern(
        strings.get_message(key).unwrap().value().unwrap(),
        Some(&args),
        &mut trash
    ).to_string()
}

// DRAW
fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    let vertical = Layout::vertical([Constraint::Percentage(100)]);
    let [content] = vertical.areas(area);

    let page_text = if let Some(s) = &app.page_display {
        s.to_owned()
    } else {
        naive_fluent(&app.strings, "welcome")
    };

    let page = Paragraph::new(page_text)
        //.centered()
        .wrap(Wrap { trim: false })
        .scroll((app.page_scroll as u16, 0));

    {
        let mut area = content;
        if app.status_display.is_some() || app.should_quit { area.height -= 1; }
        f.render_widget(page, area);
    }
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

    {
        // FIXME: Last-moment quit override kinda looks bad but if we take awhile to quit (currently common), it's needed
        let status_display = if !app.should_quit { &app.status_display} else { &Some((StatusStyle::Info, naive_fluent(&app.strings, "quitting"))) };

        if let Some((style, text)) = status_display {
            let bar = Paragraph::new(text.clone());
            let bar = bar.style(match style {
                StatusStyle::Error => Style::default().fg(Color::LightRed).add_modifier(Modifier::REVERSED),
                _ => Style::default().add_modifier(Modifier::REVERSED),
            });
            let mut area = content;
            area.y = area.height-1;
            area.height=1;
            f.render_widget(bar, area);
        }
    }

    #[cfg(feature = "debug_mode")]
    if let Some(d) = &app.debug_display {
        if let Some(text) = d.queue.front() {
            let text = format!("{text} ({})", d.queue.len());
            let bar = Paragraph::new(text.clone());
            let mut area = content;
            area.y = area.height-1;
            if app.status_display.is_some() {
                area.y -= 1;
            }
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
