use crate::renderer::gl;
use bytemuck::{Pod, Zeroable};
use glam::Vec4;

pub const ATTR_LOC_POSITION: gl::types::GLuint = 0;
pub const ATTR_LOC_NORMAL: gl::types::GLuint = 1;
pub const ATTR_LOC_TANGENT: gl::types::GLuint = 2;
pub const ATTR_LOC_TEXCOORD_0: gl::types::GLuint = 3;
pub const ATTR_LOC_TEXCOORD_1: gl::types::GLuint = 4;
pub const ATTR_LOC_COLOR_0: gl::types::GLuint = 5;
pub const ATTR_LOC_MODEL_TRANSFORM_COLUMNS: [gl::types::GLuint; 4] = [6, 7, 8, 9];
pub const ATTR_LOC_TEXCOORD_TRANSFORM_COLUMNS: [gl::types::GLuint; 4] = [10, 11, 12, 13];

pub const TEX_UNIT_BASE_COLOR: u32 = 0;
pub const TEX_UNIT_METALLIC_ROUGHNESS: u32 = 1;
pub const TEX_UNIT_NORMAL: u32 = 2;
pub const TEX_UNIT_OCCLUSION: u32 = 3;
pub const TEX_UNIT_EMISSIVE: u32 = 4;

pub const UNIFORM_BLOCK_MATERIAL: u32 = 0;
pub const UNIFORM_BLOCK_LIGHTS: u32 = 1;

pub const MAX_LIGHTS: usize = 32;

#[derive(Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct UniformBlockMaterial {
    pub base_color_factor: Vec4,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub normal_scale: f32,
    pub occlusion_strength: f32,
    pub emissive_factor: Vec4,
}

#[derive(Clone, Copy, PartialEq, Zeroable, Pod)]
#[repr(C)]
pub struct UniformBlockLights {
    /// w: 0.0 as the null terminator, 1.0: directional, 2.0: point, 3.0: spot,
    /// xyz: rgb
    pub color_and_kind: [Vec4; MAX_LIGHTS],
    /// x: intensity, y: angle scale, z: angle offset
    pub intensity_params: [Vec4; MAX_LIGHTS],
    pub position: [Vec4; MAX_LIGHTS],
    pub direction: [Vec4; MAX_LIGHTS],
}

pub struct ShaderProgram {
    pub program: gl::types::GLuint,
    pub proj_from_view_location: gl::types::GLint,
    pub view_from_world_location: gl::types::GLint,
}

/// Compiles and returns the shader program which should be used to render the
/// glTF models.
pub fn create_program() -> ShaderProgram {
    let vertex_shader = gl::create_shader(gl::VERTEX_SHADER, include_str!("gltf_vertex.glsl"));
    let fragment_shader =
        gl::create_shader(gl::FRAGMENT_SHADER, include_str!("gltf_fragment.glsl"));
    let program = gl::create_program(&[vertex_shader, fragment_shader]);
    gl::call!(gl::DeleteShader(vertex_shader));
    gl::call!(gl::DeleteShader(fragment_shader));
    gl::call!(gl::UseProgram(program));
    let proj_from_view_location = gl::get_uniform_location(program, "proj_from_view").unwrap();
    let view_from_world_location = gl::get_uniform_location(program, "view_from_world").unwrap();

    if let Some(location) = gl::get_uniform_location(program, "base_color_tex") {
        gl::call!(gl::Uniform1i(location, TEX_UNIT_BASE_COLOR as i32));
    }
    if let Some(location) = gl::get_uniform_location(program, "metallic_roughness_tex") {
        gl::call!(gl::Uniform1i(location, TEX_UNIT_METALLIC_ROUGHNESS as i32,));
    }
    if let Some(location) = gl::get_uniform_location(program, "normal_tex") {
        gl::call!(gl::Uniform1i(location, TEX_UNIT_NORMAL as i32));
    }
    if let Some(location) = gl::get_uniform_location(program, "occlusion_tex") {
        gl::call!(gl::Uniform1i(location, TEX_UNIT_OCCLUSION as i32));
    }
    if let Some(location) = gl::get_uniform_location(program, "emissive_tex") {
        gl::call!(gl::Uniform1i(location, TEX_UNIT_EMISSIVE as i32));
    }
    if let Some(loc) = gl::get_uniform_block_index(program, "Material") {
        let binding = UNIFORM_BLOCK_MATERIAL;
        gl::call!(gl::UniformBlockBinding(program, loc, binding));
    }
    if let Some(loc) = gl::get_uniform_block_index(program, "Lights") {
        let binding = UNIFORM_BLOCK_LIGHTS;
        gl::call!(gl::UniformBlockBinding(program, loc, binding));
    }

    ShaderProgram {
        program,
        proj_from_view_location,
        view_from_world_location,
    }
}
