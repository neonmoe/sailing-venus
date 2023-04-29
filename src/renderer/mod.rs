use crate::ship_game::{RoomType, ShipGame};
use glam::{Mat4, Quat, Vec2, Vec3, Vec3Swizzles, Vec4, Vec4Swizzles};
use std::f32::consts::TAU;

mod bumpalloc_buffer;
mod camera;
mod draw_calls;
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
    camera: camera::Camera,
    ship: gltf::Gltf,
    room: gltf::Gltf,
    character: gltf::Gltf,
}

impl Renderer {
    pub fn new() -> Renderer {
        let gltf_shader = gltf::create_program();
        let draw_calls = DrawCalls::new();
        let ship = gltf::load_glb(include_bytes!("../../resources/models/ship.glb"));
        let room = gltf::load_glb(include_bytes!("../../resources/models/room.glb"));
        let character = gltf::load_glb(include_bytes!("../../resources/models/character.glb"));
        Renderer {
            gltf_shader,
            draw_calls,
            camera: camera::Camera::new(),
            ship,
            room,
            character,
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

    pub fn render(&mut self, aspect_ratio: f32, time: f32, ship_game: &ShipGame) {
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

        gl::call!(gl::ClearColor(0.1, 0.1, 0.1, 1.0));
        gl::call!(gl::ClearDepthf(0.0));
        gl::call!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        gl::call!(gl::Enable(gl::CULL_FACE));
        gl::call!(gl::Enable(gl::DEPTH_TEST));
        gl::call!(gl::DepthFunc(gl::GREATER));

        let (view_matrix, proj_matrix) = self.get_view_and_proj_matrices(aspect_ratio);
        let view_matrix = view_matrix.to_cols_array();
        let proj_matrix = proj_matrix.to_cols_array();

        // Draw glTFs:
        gl::call!(gl::UseProgram(self.gltf_shader.program));
        gl::call!(gl::UniformMatrix4fv(
            self.gltf_shader.proj_from_view_location,
            1,
            gl::FALSE,
            proj_matrix.as_ptr(),
        ));
        gl::call!(gl::UniformMatrix4fv(
            self.gltf_shader.view_from_world_location,
            1,
            gl::FALSE,
            view_matrix.as_ptr(),
        ));
        self.draw_calls.draw(gltf::ATTR_LOC_MODEL_TRANSFORM_COLUMNS);
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
