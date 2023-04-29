use std::f32::consts::TAU;

use glam::{Mat4, Quat, Vec3, Vec4};

mod bumpalloc_buffer;
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
    test: gltf::Gltf,
}

impl Renderer {
    pub fn new() -> Renderer {
        let gltf_shader = gltf::create_program();
        let draw_calls = DrawCalls::new();
        let test = gltf::load_glb(include_bytes!("../../resources/models/character.glb"));
        Renderer {
            gltf_shader,
            draw_calls,
            test,
        }
    }

    pub fn render(&mut self, aspect_ratio: f32, time: f32) {
        self.draw_calls.clear();
        self.test.draw(
            &mut self.draw_calls,
            Mat4::from_rotation_translation(
                Quat::from_rotation_y(TAU * 5.0 / 8.0),
                Vec3::new(0.0, -1.0, 4.0),
            ),
        );

        gl::call!(gl::ClearColor(0.1, 0.1, 0.1, 1.0));
        gl::call!(gl::ClearDepthf(0.0));
        gl::call!(gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT));
        gl::call!(gl::Enable(gl::CULL_FACE));
        gl::call!(gl::Enable(gl::DEPTH_TEST));
        gl::call!(gl::DepthFunc(gl::GREATER));

        let view_matrix = Mat4::IDENTITY.to_cols_array();
        // OpenGL clip space: right-handed, +X right, +Y up, +Z backward (out of screen).
        // GLTF:              right-handed, +X left, +Y up, +Z forward (into the screen).
        let to_opengl_basis = Mat4::from_cols(
            (RIGHT, 0.0).into(),    // +X is right in OpenGL clip space
            (UP, 0.0).into(),       // +Y is up in OpenGL clip space
            (-FORWARD, 0.0).into(), // +Z is backward in OpenGL clip space
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let proj_matrix = (Mat4::perspective_rh_gl(74f32.to_radians(), aspect_ratio, 100.0, 0.3)
            * to_opengl_basis)
            .to_cols_array();

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
}
