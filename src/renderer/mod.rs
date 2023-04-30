use crate::ship_game::{RoomType, ShipGame};
use fontdue::layout::{HorizontalAlign, VerticalAlign};
use glam::{Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
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

    ship: gltf::Gltf,
    room: gltf::Gltf,
    character: gltf::Gltf,
    dashboard: gltf::Gltf,
}

impl Renderer {
    pub fn new() -> Renderer {
        let ship = gltf::load_glb(include_bytes!("../../resources/models/ship.glb"));
        let room = gltf::load_glb(include_bytes!("../../resources/models/room.glb"));
        let character = gltf::load_glb(include_bytes!("../../resources/models/sailor.glb"));
        let dashboard = gltf::load_glb(include_bytes!("../../resources/models/dashboard.glb"));
        Renderer {
            gltf_shader: gltf::create_program(),
            draw_calls: DrawCalls::new(),
            ui_draw_calls: DrawCalls::new(),
            camera: camera::Camera::new(),
            text: font_renderer::FontRenderer::new(),
            ship,
            room,
            character,
            dashboard,
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
        self.camera.yaw += x as f32 * 0.01;
        self.camera.pitch =
            (self.camera.pitch + y as f32 * 0.01).clamp(30.0 / 360.0 * TAU, 90.0 / 360.0 * TAU);
    }

    pub fn zoom_camera(&mut self, pixels: i32) {
        // TODO: Add camera zoom sensitivity
        self.camera.distance = (self.camera.distance - pixels as f32 * 10.0).clamp(10.0, 50.0);
    }

    pub fn render(&mut self, width: f32, height: f32, _time: f32, ship_game: &ShipGame) {
        // Render world:

        self.draw_calls.clear();
        for room in &ship_game.rooms {
            let position = Vec3::new(room.position.x, 0.0, room.position.y);
            match room.room_type {
                RoomType::Empty => {}
                RoomType::Navigation => {
                    self.room
                        .draw(&mut self.draw_calls, Mat4::from_translation(position));
                }
            }
        }
        for character in &ship_game.characters {
            let position = Vec3::new(character.position.x, 0.0, character.position.y);
            let rot = character.look_dir.angle_between(Vec2::Y);
            self.character.draw(
                &mut self.draw_calls,
                Mat4::from_rotation_translation(Quat::from_rotation_y(rot), position),
            );
        }
        self.ship.draw(&mut self.draw_calls, Mat4::IDENTITY);

        gl::call!(gl::Disable(gl::BLEND));
        gl::call!(gl::ClearColor(0.1, 0.1, 0.1, 1.0));
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

        // TODO: draw_call collect, proj+view matrix, draw_call draw just for the UI
        // TODO: 2D UI can just use glTF and an orthographic projection
        self.dashboard.draw(&mut self.ui_draw_calls, Mat4::IDENTITY);

        for (i, text) in ["NAVIGATION", "SCHEDULE", "DELIVERIES", "OPTIONS"]
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
                (HorizontalAlign::Left, VerticalAlign::Middle),
                None,
            );
        }

        self.text.draw_text(
            &mut self.ui_draw_calls,
            &format!("Some text here too. The scale is currently: {scale:.1} with width {width}"),
            Vec2::new(-30.0, 125.0),
            9.0,
            (16.0, scale),
            (HorizontalAlign::Left, VerticalAlign::Top),
            Some(320.0),
        );

        // Text rendering:
        // TODO: Collect a bunch of strings, font indices and render positions to render as text
        // TODO: Update texture with whatever needs updating
        // TODO: Add a per-instance vertex attribute with a vec4 that contains texcoord bounds
        // TODO: Render a single quad for each character, using the model transform for position & above for glyph selection
        // the quad can be defined completely ahead-of-time as a single triangle, pretty sure

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
            Mat4::perspective_rh_gl(20f32.to_radians(), aspect_ratio, 100.0, 0.3) * to_opengl_basis;
        (view_matrix, proj_matrix)
    }
}
