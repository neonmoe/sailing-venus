use anyhow::Context;
use glam::Vec2;
use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::mouse::{MouseButton, MouseWheelDirection};
use sdl2::rect::Point;
use sdl2::sys::{SDL_Event, SDL_EventType, SDL_KeyCode};
use sdl2::video::{GLProfile, Window};
use sdl2::EventPump;
use std::error::Error;
use std::ffi::{c_int, c_void};
use std::fmt::Display;
use std::panic;
use std::ptr;
use std::time::Instant;

#[cfg(target_family = "wasm")]
mod emscripten_h;
mod interface;
mod math;
mod renderer;
mod ship_game;

use interface::Interface;
use renderer::Renderer;
use ship_game::ShipGame;

fn main() {
    panic::set_hook(Box::new(|panic_info| {
        display_error(panic_info);
    }));
    if let Err(err) = _main() {
        display_error(format!("{:?}", err));
    }
}

fn _main() -> anyhow::Result<()> {
    let sdl_context = sdl2::init().map_err(SdlErr).context("sdl2::init failed")?;
    let video = sdl_context
        .video()
        .map_err(SdlErr)
        .context("sdl2 video subsystem init failed")?;
    let gl_attr = video.gl_attr();
    gl_attr.set_context_profile(GLProfile::GLES);
    gl_attr.set_context_version(3, 0);
    gl_attr.set_multisample_buffers(1);
    gl_attr.set_multisample_samples(4);
    gl_attr.set_depth_size(24);
    // Linear->SRGB conversion is done in shader, thanks to lacking WebGL support.
    gl_attr.set_framebuffer_srgb_compatible(false);
    let window = video
        .window(env!("CARGO_PKG_NAME"), 948, 533)
        .resizable()
        .opengl()
        .build()
        .context("window creation failed")?;
    let _gl_context = match window
        .gl_create_context()
        .map_err(SdlErr)
        .context("gl context creation failed")
    {
        Ok(ctx) => ctx,
        #[cfg(target_family = "wasm")]
        Err(_) => return Ok(()), // This is expected and should not "crash".
        #[cfg(not(target_family = "wasm"))]
        Err(err) => return Err(err),
    };

    {
        // Set up OpenGL, draw a "loading screen"
        use renderer::gl;

        gl::load_with(|s| video.gl_get_proc_address(s) as *const core::ffi::c_void);
        video.gl_set_swap_interval(1).unwrap();
        let (w, h) = window.drawable_size();
        gl::call!(gl::Viewport(0, 0, w as i32, h as i32));

        gl::call!(gl::ClearColor(0.2, 0.4, 0.2, 1.0));
        gl::call!(gl::Clear(gl::COLOR_BUFFER_BIT));
        // TODO: Render a loading screen
        window.gl_swap_window();
    }

    let event_pump = sdl_context
        .event_pump()
        .map_err(SdlErr)
        .context("sdl event pump creation failed")?;

    // Set up an event filter to avoid too eager preventDefault()s on
    // emscripten.
    extern "C" fn event_filter(_: *mut c_void, event: *mut SDL_Event) -> c_int {
        const DROPPED: c_int = 0;
        const ACCEPTED: c_int = 1;
        if let Some(event) = unsafe { event.as_ref() } {
            const KEYDOWN: u32 = SDL_EventType::SDL_KEYDOWN as u32;
            const KEYUP: u32 = SDL_EventType::SDL_KEYUP as u32;
            match unsafe { event.type_ } {
                KEYDOWN | KEYUP => {
                    let key_event = unsafe { event.key };
                    let keycode = key_event.keysym.sym;
                    // Here, we specifically "unignore"
                    if keycode == SDL_KeyCode::SDLK_SPACE as i32 {
                        ACCEPTED
                    } else if keycode == SDL_KeyCode::SDLK_1 as i32 {
                        ACCEPTED
                    } else if keycode == SDL_KeyCode::SDLK_2 as i32 {
                        ACCEPTED
                    } else if keycode == SDL_KeyCode::SDLK_3 as i32 {
                        ACCEPTED
                    } else if keycode == SDL_KeyCode::SDLK_4 as i32 {
                        ACCEPTED
                    } else {
                        DROPPED
                    }
                }
                _ => ACCEPTED,
            }
        } else {
            ACCEPTED
        }
    }
    unsafe { sdl2::sys::SDL_SetEventFilter(Some(event_filter), ptr::null_mut()) };

    #[cfg(target_family = "wasm")]
    {
        emscripten_h::run_javascript(
            "document.getElementById('browser-support-warning').innerHTML = \"<p>Loading...</p>\"",
        );
        // Let the JS runtime get everything else (including updating the
        // loading screen and the above notice) out of the way before continuing:
        unsafe { emscripten_h::emscripten_sleep(100) };
    }

    unsafe { STATE = Some(State::new(window, event_pump)) };

    #[cfg(target_family = "wasm")]
    {
        emscripten_h::run_javascript("document.getElementById('browser-support-warning').remove()");
        emscripten_h::set_main_loop(run_frame);
    }
    #[cfg(not(target_family = "wasm"))]
    loop {
        run_frame()
    }
}

static mut STATE: Option<State> = None;

struct State {
    window: Window,
    event_pump: EventPump,
    lmouse_pressed: bool,
    rmouse_pressed: bool,
    mouse_position: Point,
    ship_space_mouse_position: Vec2,
    accumulated_mouse_rel: (i32, i32),
    renderer: Renderer,
    time: f32,
    last_frame: Instant,
    ship_game: ShipGame,
    interface: Interface,
    debug_time_speedup: bool,
}

impl State {
    pub fn new(window: Window, event_pump: EventPump) -> State {
        let renderer = Renderer::new();
        let ship_game = ShipGame::new(&renderer);
        State {
            renderer,
            window,
            event_pump,
            lmouse_pressed: false,
            rmouse_pressed: false,
            mouse_position: Point::new(0, 0),
            ship_space_mouse_position: Vec2::ZERO,
            accumulated_mouse_rel: (0, 0),
            time: 0.0,
            last_frame: Instant::now(),
            ship_game,
            interface: Interface::new(),
            debug_time_speedup: false,
        }
    }
}

extern "C" fn run_frame() {
    let State {
        event_pump,
        lmouse_pressed,
        rmouse_pressed,
        mouse_position,
        ship_space_mouse_position,
        accumulated_mouse_rel,
        renderer,
        window,
        time,
        last_frame,
        ship_game,
        interface,
        debug_time_speedup,
        ..
    } = unsafe { &mut STATE }.as_mut().unwrap();

    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. } => std::process::exit(0),
            Event::Window { win_event, .. } => match win_event {
                WindowEvent::Resized(w, h) => {
                    use renderer::gl;
                    gl::call!(gl::Viewport(0, 0, w, h));
                }
                _ => {}
            },
            Event::MouseButtonDown {
                mouse_btn, x, y, ..
            } => match mouse_btn {
                MouseButton::Left => {
                    *lmouse_pressed = true;
                    let (w, h) = window.size();
                    let mut clip_coords =
                        Vec2::new(x as f32 / w as f32, y as f32 / h as f32) * 2.0 - Vec2::ONE;
                    clip_coords.y *= -1.0;
                    *ship_space_mouse_position =
                        renderer.clip_to_ship_space(clip_coords, w as f32 / h as f32);

                    interface.click(Point::new(x, y), ship_game, false);
                }
                MouseButton::Right => *rmouse_pressed = true,
                _ => {}
            },
            Event::MouseButtonUp { mouse_btn, .. } => {
                match mouse_btn {
                    MouseButton::Left => *lmouse_pressed = false,
                    MouseButton::Right => *rmouse_pressed = false,
                    _ => {}
                }
                *accumulated_mouse_rel = (0, 0);
            }
            Event::MouseMotion {
                x,
                y,
                mut xrel,
                mut yrel,
                ..
            } => {
                *mouse_position = Point::new(x, y);
                interface.hover(*mouse_position);
                if *rmouse_pressed {
                    renderer.rotate_camera(xrel, yrel);
                }
                if *lmouse_pressed {
                    // Look movement
                    let threshold = 10i32.pow(2);
                    let (acc_x, acc_y) = accumulated_mouse_rel;
                    if acc_x.pow(2) + acc_y.pow(2) < threshold {
                        if !interface.safe_area.contains_point(*mouse_position) {
                            *acc_x += xrel;
                            *acc_y += yrel;
                        } else {
                            // Not dragging the map around, but inside safe area with left btn held:
                            interface.click(Point::new(x, y), ship_game, true);
                        }
                        // Haven't moved enough yet, don't move.
                        xrel = 0;
                        yrel = 0;
                    }
                    let (_, h) = window.size();
                    renderer.move_camera(xrel as f32 / h as f32, yrel as f32 / h as f32);
                }
            }
            Event::MouseWheel { y, direction, .. } => {
                let pixels = y
                    * (direction == MouseWheelDirection::Flipped)
                        .then_some(-1)
                        .unwrap_or(1);
                renderer.zoom_camera(pixels);
            }
            Event::KeyDown { keycode, .. } => match keycode {
                Some(Keycode::Space) => *debug_time_speedup = true,
                Some(Keycode::Num1) => interface.open_tab(0),
                Some(Keycode::Num2) => interface.open_tab(1),
                Some(Keycode::Num3) => interface.open_tab(2),
                Some(Keycode::Num4) => interface.open_tab(3),
                _ => {}
            },
            Event::KeyUp { keycode, .. } => match keycode {
                Some(Keycode::Space) => *debug_time_speedup = false,
                _ => {}
            },
            _ => {}
        }
    }

    let now = Instant::now();
    let dt = (now - *last_frame).as_secs_f32();
    *time += dt;
    *last_frame = now;

    let speed_scale = if *debug_time_speedup { 12.0 } else { 1.0 };
    ship_game.update(dt * speed_scale);

    let (w, h) = window.drawable_size();
    renderer.render(w as f32, h as f32, *time, &ship_game, interface);
    window.gl_swap_window();
}

fn display_error<D: Display>(err: D) {
    #[cfg(target_family = "wasm")]
    emscripten_h::run_javascript(
        &format!(
            "document.getElementById('browser-support-warning').innerHTML = \"<p>The game crashed! You can try the desktop version instead.</p>\
            <p><details><summary>Crash report:</summary><pre>{}</pre></details></p>\"",
            err.to_string().replace("\\", "\\\\").replace("\n", "\\n").replace("\"", "\\\""),
        ),
    );
    #[cfg(not(target_family = "wasm"))]
    {
        use sdl2::messagebox::{show_simple_message_box, MessageBoxFlag};

        eprintln!("fatal error: {err}");
        let window = unsafe { STATE.as_ref() }.map(|state| &state.window);
        let _ = show_simple_message_box(
            MessageBoxFlag::ERROR,
            "Game crashed!",
            &format!("Crash report:\n\n{err}"),
            window,
        );
    }
}

#[derive(Debug)]
pub struct SdlErr(String);
impl Display for SdlErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sdl error: {}", self.0)
    }
}
impl Error for SdlErr {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
