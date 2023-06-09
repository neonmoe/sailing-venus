use crate::{
    interface::{Button, Interface, Tab},
    ship_game::{Character, Job, RoomType, ShipGame, Task},
};
use fontdue::layout::{HorizontalAlign, VerticalAlign};
use glam::{IVec2, Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
use sdl2::rect::Rect;
use std::f32::consts::TAU;

mod bumpalloc_buffer;
mod camera;
mod draw_calls;
mod font_renderer;
pub mod gl;
pub mod gltf;

pub use draw_calls::DrawCalls;

/// The "up" vector in world-space (which is in glTF's coordinate system, for
/// now).
pub const UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
/// The "right" vector in world-space (which is in glTF's coordinate system, for
/// now).
pub const RIGHT: Vec3 = Vec3::new(-1.0, 0.0, 0.0);
/// The "forward" vector in world-space (which is in glTF's coordinate system,
/// for now).
pub const FORWARD: Vec3 = Vec3::new(0.0, 0.0, 1.0);

pub struct Renderer {
    gltf_shader: gltf::ShaderProgram,
    draw_calls: DrawCalls,
    ui_draw_calls: DrawCalls,
    camera: camera::Camera,
    text: font_renderer::FontRenderer,

    debug_arrow: gltf::Gltf,
    ship: gltf::Gltf,
    pub room_sailing: gltf::Gltf,
    pub room_navigation: gltf::Gltf,
    characters: [gltf::Gltf; Job::Count as usize],
    dashboard: gltf::Gltf,
    pixel_gray: gltf::Gltf,
    pixel_green: gltf::Gltf,
}

impl Renderer {
    pub fn new() -> Renderer {
        let debug_arrow = gltf::load_glb(include_bytes!("../../resources/models/debug_arrow.glb"));
        let ship = gltf::load_glb(include_bytes!("../../resources/models/ship.glb"));
        let room_sailing =
            gltf::load_glb(include_bytes!("../../resources/models/room_sailing.glb"));
        let room_navigation =
            gltf::load_glb(include_bytes!("../../resources/models/room_navigation.glb"));
        let navigator = gltf::load_glb(include_bytes!("../../resources/models/navigator.glb"));
        let sailor = gltf::load_glb(include_bytes!("../../resources/models/sailor.glb"));
        let dashboard = gltf::load_glb(include_bytes!("../../resources/models/dashboard.glb"));
        let pixel_gray = gltf::load_glb(include_bytes!("../../resources/models/pixel_gray.glb"));
        let pixel_green = gltf::load_glb(include_bytes!("../../resources/models/pixel_green.glb"));
        Renderer {
            gltf_shader: gltf::create_program(),
            draw_calls: DrawCalls::new(),
            ui_draw_calls: DrawCalls::new(),
            camera: camera::Camera::new(),
            text: font_renderer::FontRenderer::new(),
            debug_arrow,
            ship,
            room_sailing,
            room_navigation,
            characters: [navigator, sailor],
            dashboard,
            pixel_gray,
            pixel_green,
        }
    }

    pub fn clip_to_ship_space(&self, clip_coords: Vec2, aspect_ratio: f32) -> Vec2 {
        let clip_vec = Vec4::new(clip_coords.x, clip_coords.y, 1.0, 1.0);
        let (view, proj) = self.get_view_and_proj_matrices(aspect_ratio);
        let view_inv = view.inverse();
        let proj_inv = proj.inverse();
        let mut view_point = proj_inv * clip_vec;
        view_point /= view_point.w;
        let view_point = Vec4::from((view_point.xyz().normalize(), 0.0));
        let look_dir = (view_inv * view_point).xyz().normalize();
        let ship_coord = if look_dir.dot(Vec3::Y) >= 0.0 {
            Vec2::new(f32::INFINITY, f32::INFINITY)
        } else {
            let origin = (view_inv * Vec4::new(0.0, 0.0, 0.0, 1.0)).xyz();
            let length = (origin.y / look_dir.y).abs();
            let floor_point = origin + look_dir * length;
            floor_point.xz()
        };
        let maximum_distance = 100.0;
        ship_coord.clamp(-Vec2::ONE * maximum_distance, Vec2::ONE * maximum_distance)
    }

    pub fn move_camera(&mut self, x: f32, y: f32) {
        // TODO: Add camera move sensitivity
        let sensitivity = Vec2::ONE * 0.4 * self.camera.distance;
        let view_space_move = Vec3::new(x * sensitivity.x, 0.0, y * sensitivity.y);
        let world_space_move =
            Quat::from_rotation_y(-(self.camera.yaw + TAU / 2.0)) * view_space_move;
        // TODO: Use the bounds of all the rooms here
        self.camera.focus =
            (self.camera.focus + world_space_move).clamp(Vec3::ONE * -10.0, Vec3::ONE * 10.0);
    }

    pub fn rotate_camera(&mut self, x: i32, y: i32) {
        // TODO: Add camera rotation sensitivity
        let sensitivity = Vec2::ONE * 0.004;
        self.camera.yaw += x as f32 * sensitivity.x;
        self.camera.pitch = (self.camera.pitch + y as f32 * sensitivity.y)
            .clamp(30.0 / 360.0 * TAU, 90.0 / 360.0 * TAU);
    }

    pub fn zoom_camera(&mut self, pixels: i32) {
        // TODO: Add camera zoom sensitivity
        self.camera.distance = (self.camera.distance - pixels as f32 * 10.0).clamp(10.0, 100.0);
    }

    pub fn render(
        &mut self,
        width: f32,
        height: f32,
        _time: f32,
        ship_game: &ShipGame,
        interface: &mut Interface,
    ) {
        // Render world:

        self.draw_calls.clear();
        for room in &ship_game.rooms {
            let position = Vec3::new(room.position.x, 0.0, room.position.y);
            match room.room_type {
                RoomType::Navigation => self
                    .room_navigation
                    .draw(&mut self.draw_calls, Mat4::from_translation(position)),
                RoomType::Sails => self
                    .room_sailing
                    .draw(&mut self.draw_calls, Mat4::from_translation(position)),
            }
        }
        for character in &ship_game.characters {
            let position = Vec3::new(character.position.x, 0.0, character.position.y);
            let rot = character.look_dir.angle_between(Vec2::Y);
            self.characters[character.job as usize].draw(
                &mut self.draw_calls,
                Mat4::from_rotation_translation(Quat::from_rotation_y(rot), position),
            );
        }
        self.ship.draw(&mut self.draw_calls, Mat4::IDENTITY);

        let pathfinding_debug_arrows = false;
        if cfg!(debug_assertions) && pathfinding_debug_arrows {
            let to_3d = |vec2: &IVec2| Vec3::new(vec2.x as f32 + 0.5, 0.5, vec2.y as f32 + 0.5);
            for (debug_arrow, neighbors) in &ship_game.pf_map {
                let debug_arrow = to_3d(debug_arrow);
                self.debug_arrow
                    .draw(&mut self.draw_calls, Mat4::from_translation(debug_arrow));
                for neighbor in neighbors {
                    let diff = to_3d(neighbor) - debug_arrow;
                    self.debug_arrow.draw(
                        &mut self.draw_calls,
                        Mat4::from_scale_rotation_translation(
                            Vec3::ONE * 0.3,
                            Quat::IDENTITY,
                            debug_arrow + diff * 0.25,
                        ),
                    );
                }
            }
        }

        gl::call!(gl::Disable(gl::BLEND));
        gl::call!(gl::ClearColor(0.6, 0.45, 0.3, 1.0));
        gl::call!(gl::ClearDepthf(0.0));
        gl::call!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        gl::call!(gl::Enable(gl::CULL_FACE));
        gl::call!(gl::Enable(gl::DEPTH_TEST));
        gl::call!(gl::DepthFunc(gl::GREATER));

        let (v, p) = self.get_view_and_proj_matrices(width / height);
        let world_view_matrix = v.to_cols_array();
        let world_proj_matrix = p.to_cols_array();

        gl::call!(gl::UseProgram(self.gltf_shader.program));
        gl::call!(gl::UniformMatrix4fv(
            self.gltf_shader.proj_from_view_location,
            1,
            gl::FALSE,
            world_proj_matrix.as_ptr(),
        ));
        gl::call!(gl::UniformMatrix4fv(
            self.gltf_shader.view_from_world_location,
            1,
            gl::FALSE,
            world_view_matrix.as_ptr(),
        ));
        self.draw_calls.draw(
            gltf::ATTR_LOC_MODEL_TRANSFORM_COLUMNS,
            gltf::ATTR_LOC_TEXCOORD_TRANSFORM_COLUMNS,
        );

        // Render UI:

        let scale = (width / 800.0).floor().max(1.0);
        let width = width / scale;
        let height = height / scale;
        self.ui_draw_calls.clear();

        gl::call!(gl::Enable(gl::BLEND));
        gl::call!(gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA));
        gl::call!(gl::ClearDepthf(1.0));
        gl::call!(gl::Clear(gl::DEPTH_BUFFER_BIT));
        gl::call!(gl::DepthFunc(gl::LESS));

        // Draw dashboard & clock
        let mut dashboard_transforms = self.dashboard.get_node_transforms();
        for node_transform in &mut dashboard_transforms {
            if node_transform.name == "Clock Hand" {
                let (s, mut r, t) = node_transform.transform.to_scale_rotation_translation();
                r *= Quat::from_rotation_z(-TAU * (ship_game.world_time * 60.0).floor() / 60.0);
                node_transform.transform = Mat4::from_scale_rotation_translation(s, r, t);
            }
        }
        self.dashboard.draw_animated(
            &mut self.ui_draw_calls,
            Mat4::IDENTITY,
            &dashboard_transforms,
        );
        self.text.draw_text(
            &mut self.ui_draw_calls,
            &format!("DAY {:.0}", ship_game.world_time.floor()),
            Vec2::new(-115.0, 68.0),
            9.0,
            (11.0, scale),
            (HorizontalAlign::Center, VerticalAlign::Top),
            Some(115.0 - 68.0),
        );

        let interface_rect = |x: f32, y: f32, w: f32, h: f32| {
            Rect::new(
                ((width / 2.0 + x) * scale) as i32,
                ((height - h - y + 2.0) * scale).ceil() as i32,
                (w * scale) as u32,
                (h * scale) as u32,
            )
        };

        // Tabs
        interface.buttons.clear();
        for (i, text) in ["NAVIGATION", "SCHEDULE", "DELIVERIES", "GAME SETTINGS"]
            .iter()
            .enumerate()
        {
            let y = 132.0 - i as f32 * 29.5;
            self.text.draw_text(
                &mut self.ui_draw_calls,
                text,
                Vec2::new(-270.0, y),
                9.0,
                (20.0, scale),
                (HorizontalAlign::Left, VerticalAlign::Top),
                None,
            );
            interface.buttons.insert(
                Button::Tab(i),
                interface_rect(-300.0, y - 2.0 - 28.0, 180.0, 28.0),
            );
        }

        // Interface
        let scr_x = -39.0;
        let scr_y = 17.0;
        let scr_h = 114.0;
        interface.screen_area = interface_rect(scr_x, scr_y, 336.0, scr_h);
        interface.safe_area = interface_rect(-322.0, 0.0, 644.0, 154.0);
        match interface.tab {
            Some(Tab::Navigation) => {
                let mut draw_location = |name: &str, location: Vec2, i: usize| {
                    let x = scr_x + 10.0;
                    let y = scr_y + scr_h - i as f32 * 17.0 - 3.0;
                    self.text.draw_text(
                        &mut self.ui_draw_calls,
                        name,
                        Vec2::new(x, y),
                        5.0,
                        (14.0, scale),
                        (HorizontalAlign::Left, VerticalAlign::Top),
                        None,
                    );
                    if (location - ship_game.current_location).length() < 1.0 {
                        self.pixel_gray.draw(
                            &mut self.ui_draw_calls,
                            Mat4::from_scale_rotation_translation(
                                Vec3::new(315.0, 2.0, 2.0),
                                Quat::IDENTITY,
                                Vec3::new(x, y - 18.0, 5.0),
                            ),
                        )
                    }
                    interface.buttons.insert(
                        Button::LocationList(i),
                        interface_rect(x, y - 16.0, 300.0, 16.0),
                    );
                };
                for (i, location) in ship_game.locations.iter().enumerate() {
                    draw_location(location.0, location.1, i)
                }
                let mut target = "";
                for (name, location) in &ship_game.locations {
                    if *location == ship_game.current_target {
                        target = name;
                        break;
                    }
                }
                let spd = ship_game.current_ship_speed;
                let d = (ship_game.current_target - ship_game.current_location).length() / 3.6;
                self.text.draw_text(
                    &mut self.ui_draw_calls,
                    &format!("Heading: {target}\nSpeed: {spd:.1} m/s, distance: {d:.1} km"),
                    Vec2::new(scr_x + 10.0, scr_y + 38.0),
                    5.0,
                    (14.0, scale),
                    (HorizontalAlign::Left, VerticalAlign::Top),
                    None,
                );
            }
            Some(Tab::Schedule) => {
                let l = 16.0;
                let mut draw_legend = |pixel: &gltf::Gltf, name: &str, task: Task, x_off: f32| {
                    let x = scr_x + 10.0 + x_off;
                    let y = scr_y + scr_h - 10.0;
                    pixel.draw(
                        &mut self.ui_draw_calls,
                        Mat4::from_scale_rotation_translation(
                            Vec3::new(l, l, 1.0),
                            Quat::IDENTITY,
                            Vec3::new(x, y - l, 5.0),
                        ),
                    );
                    self.text.draw_text(
                        &mut self.ui_draw_calls,
                        name,
                        Vec2::new(x + 20.0, y + 2.0),
                        5.0,
                        (14.0, scale),
                        (HorizontalAlign::Left, VerticalAlign::Top),
                        None,
                    );
                    if interface.selected_task == task {
                        pixel.draw(
                            &mut self.ui_draw_calls,
                            Mat4::from_scale_rotation_translation(
                                Vec3::new(50.0, 2.0, 2.0),
                                Quat::IDENTITY,
                                Vec3::new(x + 18.0, y - l, 5.0),
                            ),
                        )
                    }
                    interface.buttons.insert(
                        Button::TaskPicker(task),
                        interface_rect(x, y - l - 5.0, 70.0, l + 10.0),
                    );
                };
                draw_legend(&self.pixel_gray, "Sleep", Task::Sleep, 0.0);
                draw_legend(&self.pixel_green, "Work", Task::Work, 85.0);

                let mut draw_schedule = |char_idx: usize, character: &Character, y_offset: f32| {
                    let x = scr_x + 18.0;
                    let y = scr_y + 32.0 + y_offset;
                    self.characters[character.job as usize].draw(
                        &mut self.ui_draw_calls,
                        Mat4::from_scale_rotation_translation(
                            Vec3::ONE * 16.0,
                            Quat::IDENTITY,
                            Vec3::new(x, y - 22.0, 5.0),
                        ),
                    );
                    for i in 0..12 {
                        let pixel = match character.schedule[i] {
                            Task::Sleep => &self.pixel_gray,
                            Task::Work => &self.pixel_green,
                        };
                        let x = x + 16.0 + 20.0 * i as f32;
                        pixel.draw(
                            &mut self.ui_draw_calls,
                            Mat4::from_scale_rotation_translation(
                                Vec3::new(l, l, 1.0),
                                Quat::IDENTITY,
                                Vec3::new(x, y - l, 5.0),
                            ),
                        );
                        interface.buttons.insert(
                            Button::TaskAssigner {
                                character: char_idx,
                                time: i,
                            },
                            interface_rect(x, y - l - 5.0, l, l + 10.0),
                        );
                    }
                };
                for (i, character) in ship_game.characters.iter().enumerate() {
                    draw_schedule(i, character, i as f32 * 40.0);
                }
            }
            Some(Tab::Deliveries) => {
                let mut draw_delivery = |name: &str, done: bool, i: usize| {
                    let x = scr_x + 10.0;
                    let y = scr_y + scr_h - i as f32 * 25.0 - 10.0;
                    let check = if done { "x" } else { "  " };
                    self.text.draw_text(
                        &mut self.ui_draw_calls,
                        &format!("[{check}] {name}"),
                        Vec2::new(x, y),
                        5.0,
                        (20.0, scale),
                        (HorizontalAlign::Left, VerticalAlign::Top),
                        None,
                    );
                };
                let mut checks = 0;
                for (i, delivery) in ship_game.deliveries.iter().enumerate() {
                    draw_delivery(delivery.0, delivery.2, i);
                    if delivery.2 {
                        checks += 1;
                    }
                }
                if checks == ship_game.deliveries.len() {
                    self.text.draw_text(
                        &mut self.ui_draw_calls,
                        "Well done, you delivered all the packages!",
                        Vec2::new(scr_x + 7.0, scr_y + 30.0),
                        5.0,
                        (14.0, scale),
                        (HorizontalAlign::Left, VerticalAlign::Top),
                        None,
                    );
                }
            }
            Some(Tab::GameSettings) => {}
            _ => {}
        }

        let ui_proj_matrix =
            Mat4::orthographic_rh_gl(-width / 2.0, width / 2.0, 0.0, height, -100.0, 100.0)
                .to_cols_array();
        let ui_view_matrix = Mat4::IDENTITY.to_cols_array();
        gl::call!(gl::UseProgram(self.gltf_shader.program));
        gl::call!(gl::UniformMatrix4fv(
            self.gltf_shader.proj_from_view_location,
            1,
            gl::FALSE,
            ui_proj_matrix.as_ptr(),
        ));
        gl::call!(gl::UniformMatrix4fv(
            self.gltf_shader.view_from_world_location,
            1,
            gl::FALSE,
            ui_view_matrix.as_ptr(),
        ));
        self.ui_draw_calls.draw(
            gltf::ATTR_LOC_MODEL_TRANSFORM_COLUMNS,
            gltf::ATTR_LOC_TEXCOORD_TRANSFORM_COLUMNS,
        );
    }

    fn get_view_and_proj_matrices(&self, aspect_ratio: f32) -> (Mat4, Mat4) {
        // OpenGL clip space: right-handed, +X right, +Y up, +Z backward (out of screen).
        // GLTF:              right-handed, +X left, +Y up, +Z forward (into the screen).
        let to_opengl_basis = Mat4::from_cols(
            (RIGHT, 0.0).into(),    // +X is right in OpenGL clip space
            (UP, 0.0).into(),       // +Y is up in OpenGL clip space
            (-FORWARD, 0.0).into(), // +Z is backward in OpenGL clip space
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );

        let view_matrix = self.camera.view_matrix();
        let proj_matrix =
            Mat4::perspective_rh_gl(20f32.to_radians(), aspect_ratio, 200.0, 0.3) * to_opengl_basis;
        (view_matrix, proj_matrix)
    }
}
