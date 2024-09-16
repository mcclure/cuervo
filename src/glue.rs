// servo_glue / cuervo_glue / something
// Modeled on: https://github.com/servo/servo/blob/main/ports/servoshell/egl/host_trait.rs
// Modeled on: https://github.com/servo/servo/blob/main/ports/servoshell/egl/servo_glue.rs
// Modeled on: https://github.com/servo/servo/blob/main/ports/servoshell/egl/android/simpleservo.rs

use std::cell::RefCell;
use servo::embedder_traits::{InputMethodType, MediaSessionPlaybackState, PromptResult};
use servo::webrender_api::units::DeviceIntRect;
use servo::euclid::{Point2D, Rect, Scale, Size2D};
use servo::style_traits::DevicePixel;
use servo::webrender_traits::RenderingContext;
use servo::compositing::windowing::{
    AnimationState, EmbedderCoordinates, WindowMethods,
};

// host_trait

/// Callbacks. Implemented by embedder. Called by Servo.
pub trait HostTrait {
    /// Show alert.
    // fn prompt_alert(&self, msg: String, trusted: bool);
    // /// Ask Yes/No question.
    // fn prompt_yes_no(&self, msg: String, trusted: bool) -> PromptResult;
    // /// Ask Ok/Cancel question.
    // fn prompt_ok_cancel(&self, msg: String, trusted: bool) -> PromptResult;
    // /// Ask for string
    // fn prompt_input(&self, msg: String, default: String, trusted: bool) -> Option<String>;
    // /// Show context menu
    // fn show_context_menu(&self, title: Option<String>, items: Vec<String>);
    // /// Page starts loading.
    // /// "Reload button" should be disabled.
    // /// "Stop button" should be enabled.
    // /// Throbber starts spinning.
    // fn on_load_started(&self);
    // /// Page has loaded.
    // /// "Reload button" should be enabled.
    // /// "Stop button" should be disabled.
    // /// Throbber stops spinning.
    // fn on_load_ended(&self);
    // /// Page title has changed.
    // fn on_title_changed(&self, title: Option<String>);
    // /// Allow Navigation.
    // fn on_allow_navigation(&self, url: String) -> bool;
    // /// Page URL has changed.
    // fn on_url_changed(&self, url: String);
    // /// Back/forward state has changed.
    // /// Back/forward buttons need to be disabled/enabled.
    // fn on_history_changed(&self, can_go_back: bool, can_go_forward: bool);
    // /// Page animation state has changed. If animating, it's recommended
    // /// that the embedder doesn't wait for the wake function to be called
    // /// to call perform_updates. Usually, it means doing:
    // /// ```rust
    // /// while true {
    // ///     servo.perform_updates();
    // ///     servo.present_if_needed();
    // /// }
    /// ```
    /// . This will end up calling flush
    /// which will call swap_buffer which will be blocking long enough to limit
    /// drawing at 60 FPS.
    /// If not animating, call perform_updates only when needed (when the embedder
    /// has events for Servo, or Servo has woken up the embedder event loop via
    /// EventLoopWaker).
    fn on_animating_changed(&self, animating: bool);
    // /// Servo finished shutting down.
    // fn on_shutdown_complete(&self);
    // /// A text input is focused.
    // fn on_ime_show(
    //     &self,
    //     input_type: InputMethodType,
    //     text: Option<(String, i32)>,
    //     multiline: bool,
    //     bounds: DeviceIntRect,
    // );
    // /// Input lost focus
    // fn on_ime_hide(&self);
    // /// Gets sytem clipboard contents.
    // fn get_clipboard_contents(&self) -> Option<String>;
    // /// Sets system clipboard contents.
    // fn set_clipboard_contents(&self, contents: String);
    // /// Called when we get the media session metadata/
    // fn on_media_session_metadata(&self, title: String, artist: String, album: String);
    // /// Called when the media session playback state changes.
    // fn on_media_session_playback_state_change(&self, state: MediaSessionPlaybackState);
    // /// Called when the media session position state is set.
    // fn on_media_session_set_position_state(&self, duration: f64, position: f64, playback_rate: f64);
    // /// Called when devtools server is started
    // fn on_devtools_started(&self, port: Result<u16, ()>, token: String);
    // /// Called when we get a panic message from constellation
    // fn on_panic(&self, reason: String, backtrace: Option<String>);
}

// servo_glue

#[derive(Clone, Debug)]
pub struct Coordinates {
    pub viewport: Rect<i32, DevicePixel>,
    pub framebuffer: Size2D<i32, DevicePixel>,
}

impl Coordinates {
    pub fn new(
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        fb_width: i32,
        fb_height: i32,
    ) -> Coordinates {
        Coordinates {
            viewport: Rect::new(Point2D::new(x, y), Size2D::new(width, height)),
            framebuffer: Size2D::new(fb_width, fb_height),
        }
    }
}

pub struct WindowCallbacks {
    host_callbacks: Box<dyn HostTrait>,
    coordinates: RefCell<Coordinates>,
    density: f32,
    rendering_context: RenderingContext,
}

impl WindowCallbacks {
    pub(super) fn new(
        host_callbacks: Box<dyn HostTrait>,
        coordinates: RefCell<Coordinates>,
        density: f32,
        rendering_context: RenderingContext,
    ) -> Self {
        Self {
            host_callbacks,
            coordinates,
            density,
            rendering_context,
        }
    }
}

impl WindowMethods for WindowCallbacks {
    fn get_coordinates(&self) -> EmbedderCoordinates {
        let coords = self.coordinates.borrow();
        EmbedderCoordinates {
            viewport: coords.viewport.to_box2d(),
            framebuffer: coords.framebuffer,
            window: (coords.viewport.size, Point2D::new(0, 0)),
            screen: coords.viewport.size,
            screen_avail: coords.viewport.size,
            hidpi_factor: Scale::new(self.density),
        }
    }

    fn set_animation_state(&self, state: AnimationState) {
        //debug!("WindowMethods::set_animation_state: {:?}", state);
        self.host_callbacks
            .on_animating_changed(state == AnimationState::Animating);
    }

    fn rendering_context(&self) -> RenderingContext {
        self.rendering_context.clone()
    }
}
