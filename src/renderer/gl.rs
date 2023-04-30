include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// A wrapper for OpenGL calls, wrapping the call inside unsafe {} and possibly
/// panicing based on glGetError in debug builds.
macro_rules! call {
    ($expr:expr) => {{
        let result = unsafe { $expr };
        if cfg!(debug_assertions) {
            use crate::renderer::gl::*;
            let error = unsafe { GetError() };
            if error != NO_ERROR {
                let error_number_stringified;
                let error_name = match error {
                    INVALID_ENUM => "INVALID_ENUM",
                    INVALID_VALUE => "INVALID_VALUE",
                    INVALID_OPERATION => "INVALID_OPERATION",
                    OUT_OF_MEMORY => "OUT_OF_MEMORY",
                    INVALID_FRAMEBUFFER_OPERATION => "INVALID_FRAMEBUFFER_OPERATION",
                    _ => {
                        error_number_stringified = format!("{error}");
                        &error_number_stringified
                    }
                };
                panic!(
                    "OpenGL error {error_name} at {}:{}:{}",
                    file!(),
                    line!(),
                    column!(),
                );
            }
        }
        result
    }};
}
pub(crate) use call;

use std::ffi::{c_void, CString};

#[track_caller]
pub fn create_shader(type_: types::GLenum, shader_source: &str) -> u32 {
    let shader = call!(CreateShader(type_));
    let sources = [shader_source.as_bytes().as_ptr() as *const i8];
    let source_lengths = [shader_source.len() as i32];
    call!(ShaderSource(
        shader,
        1,
        sources.as_ptr(),
        source_lengths.as_ptr(),
    ));
    call!(CompileShader(shader));
    let mut compile_status = 0;
    call!(GetShaderiv(shader, COMPILE_STATUS, &mut compile_status));
    if compile_status == FALSE as i32 {
        let mut info_log = [0u8; 4096];
        let mut length = 0;
        call!(GetShaderInfoLog(
            shader,
            4096,
            &mut length,
            info_log.as_mut_ptr() as *mut i8,
        ));
        let info_log = std::str::from_utf8(&info_log[..length as usize]).unwrap();
        let shader_type = match type_ {
            VERTEX_SHADER => "Vertex ",
            FRAGMENT_SHADER => "Fragment ",
            _ => "",
        };
        panic!("{shader_type}shader compilation failed: {info_log}",);
    }
    shader
}

#[track_caller]
pub fn create_program(shaders: &[u32]) -> u32 {
    let program = call!(CreateProgram());
    for shader in shaders {
        call!(AttachShader(program, *shader));
    }
    call!(LinkProgram(program));
    let mut link_status = 0;
    call!(GetProgramiv(program, LINK_STATUS, &mut link_status));
    if link_status == FALSE as i32 {
        let mut info_log = [0u8; 4096];
        let mut length = 0;
        call!(GetProgramInfoLog(
            program,
            4096,
            &mut length,
            info_log.as_mut_ptr() as *mut i8,
        ));
        let info_log = std::str::from_utf8(&info_log[..length as usize]).unwrap();
        panic!("Linking shader program failed: {info_log}");
    }
    program
}

pub fn get_uniform_location(program: u32, name: &str) -> Option<i32> {
    let name = CString::from_vec_with_nul(format!("{name}\0").into_bytes()).unwrap();
    let location = call!(GetUniformLocation(program, name.as_ptr()));
    if location == -1 {
        None
    } else {
        Some(location)
    }
}

pub fn get_uniform_block_index(program: u32, name: &str) -> Option<u32> {
    let name = CString::from_vec_with_nul(format!("{name}\0").into_bytes()).unwrap();
    let location = call!(GetUniformBlockIndex(program, name.as_ptr()));
    if location == INVALID_INDEX {
        None
    } else {
        Some(location)
    }
}

pub fn setup_linear_sampler(sampler: u32, mipmaps: bool) {
    call!(SamplerParameteri(
        sampler,
        TEXTURE_MAG_FILTER,
        LINEAR as i32,
    ));
    if mipmaps {
        call!(SamplerParameteri(
            sampler,
            TEXTURE_MIN_FILTER,
            LINEAR_MIPMAP_LINEAR as i32,
        ));
    } else {
        call!(SamplerParameteri(
            sampler,
            TEXTURE_MIN_FILTER,
            LINEAR as i32,
        ));
    }
    call!(SamplerParameteri(sampler, TEXTURE_WRAP_S, REPEAT as i32,));
    call!(SamplerParameteri(sampler, TEXTURE_WRAP_T, REPEAT as i32,));
}

pub fn write_1px_rgb_texture(tex: u32, color: [u8; 3]) {
    let target = TEXTURE_2D;
    let ifmt = RGB as i32;
    let fmt = RGB;
    let type_ = UNSIGNED_BYTE;
    let pixels = color.as_ptr() as *const c_void;
    call!(BindTexture(target, tex));
    call!(TexImage2D(target, 0, ifmt, 1, 1, 0, fmt, type_, pixels));
}
