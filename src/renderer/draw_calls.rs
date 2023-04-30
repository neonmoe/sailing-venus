use crate::renderer::bumpalloc_buffer::BumpAllocatedBuffer;
use crate::renderer::{gl, gltf};
use bytemuck::Zeroable;
use glam::{Mat4, Vec4};
use std::collections::HashMap;
use std::ffi::c_void;
use std::{mem, ptr};

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Uniforms {
    /// The OpenGL textures to bind at GL_TEXTURE0 + i where each element is
    /// of this array is `(i, texture_object, sampler_object)`.
    pub textures: [Option<(u32, u32, u32)>; 5],
    /// The OpenGL uniform buffers `buffer` to bind at indices `i`, where each
    /// element of this array is `(i, buffer, offset, size)`.
    pub ubos: [Option<(u32, u32, usize, usize)>; 1],
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DrawCall {
    pub vao: gl::types::GLuint,
    pub mode: gl::types::GLenum,
    pub index_buffer: gl::types::GLuint,
    pub index_type: gl::types::GLuint,
    pub index_byte_offset: usize,
    pub index_count: gl::types::GLint,
    /// If vertex colors aren't provided, they should default to 1, 1, 1, 1
    /// instead of the default 0, 0, 0, 1. The problem is, the default value
    /// needs to be provided at draw-time, and can't be saved in the VAO. So
    /// this holds the location of the vertex color attribute, if it's disabled.
    pub disabled_all_ones_vertex_attribute: Option<gl::types::GLuint>,
    pub front_face: gl::types::GLenum,
}

#[derive(Default)]
struct InstanceData {
    transforms: Vec<Mat4>,
    texcoord_transforms: Vec<Mat4>,
    count: gl::types::GLsizei,
}

/// Stores the required information for rendering a set of primitives with
/// various materials, in a form that's optimized for minimum state changes
/// during rendering.
pub struct DrawCalls {
    draws: HashMap<Uniforms, HashMap<DrawCall, InstanceData>>,
    temp_buffer: BumpAllocatedBuffer,
    lights_ubo: gltf::UniformBlockLights,
    lights_count: usize,
}

impl DrawCalls {
    pub fn new() -> DrawCalls {
        DrawCalls {
            draws: HashMap::new(),
            temp_buffer: BumpAllocatedBuffer::new(gl::ARRAY_BUFFER, gl::STREAM_DRAW),
            lights_ubo: gltf::UniformBlockLights::zeroed(),
            lights_count: 0,
        }
    }

    pub fn add(
        &mut self,
        lights: Option<&gltf::UniformBlockLights>,
        uniforms: &Uniforms,
        draw_call: &DrawCall,
        model_transfrom: Mat4,
        primitive_transfrom: Mat4,
        texcoord_transform: Mat4,
    ) {
        if let Some(lights) = lights {
            for i in 0..gltf::MAX_LIGHTS {
                if lights.color_and_kind[i].w == 0.0 {
                    break;
                }
                let light_position = model_transfrom * lights.position[i];
                let light_direction = model_transfrom * lights.direction[i];
                let mut included = false;
                for j in 0..self.lights_count {
                    if lights.color_and_kind[i] == self.lights_ubo.color_and_kind[j]
                        && light_position == self.lights_ubo.position[j]
                        && lights.intensity_params[i] == self.lights_ubo.intensity_params[j]
                        && light_direction == self.lights_ubo.direction[j]
                    {
                        included = true;
                        break;
                    }
                }
                if !included {
                    if self.lights_count >= gltf::MAX_LIGHTS {
                        debug_assert!(false, "scene lights overflowed");
                        break;
                    }
                    self.lights_ubo.color_and_kind[self.lights_count] = lights.color_and_kind[i];
                    self.lights_ubo.intensity_params[self.lights_count] =
                        lights.intensity_params[i];
                    self.lights_ubo.position[self.lights_count] = light_position;
                    self.lights_ubo.direction[self.lights_count] = light_direction;
                    self.lights_count += 1;
                }
            }
        }
        let draw = if let Some(draw) = self.draws.get_mut(uniforms) {
            draw
        } else {
            self.draws.entry(uniforms.clone()).or_default()
        };
        let mut draw_call = if let Some(draw_call) = draw.get_mut(draw_call) {
            draw_call
        } else {
            draw.entry(draw_call.clone()).or_default()
        };
        draw_call.count += 1;
        draw_call.transforms.push(primitive_transfrom);
        draw_call.texcoord_transforms.push(texcoord_transform);
    }

    pub fn draw(
        &mut self,
        model_transform_attrib_locations: [u32; 4],
        texcoord_transform_attrib_locations: [u32; 4],
    ) {
        let lights = [self.lights_ubo];
        let lights = bytemuck::cast_slice(&lights);
        let (lights_buf, lights_off) = self.temp_buffer.allocate_buffer(lights);
        gl::call!(gl::BindBufferRange(
            gl::UNIFORM_BUFFER,
            gltf::UNIFORM_BLOCK_LIGHTS,
            lights_buf,
            lights_off as isize,
            lights.len() as isize,
        ));

        for (uniforms, draw_calls) in &self.draws {
            let empty_draw = draw_calls
                .values()
                .all(|instance| instance.transforms.is_empty());
            if empty_draw {
                continue;
            }

            for (binding, texture, sampler) in uniforms.textures.iter().flatten() {
                gl::call!(gl::ActiveTexture(
                    gl::TEXTURE0 + *binding as gl::types::GLenum
                ));
                gl::call!(gl::BindTexture(gl::TEXTURE_2D, *texture));
                gl::call!(gl::BindSampler(*binding as u32, *sampler));
            }

            for &(index, buffer, offset, size) in uniforms.ubos.iter().flatten() {
                gl::call!(gl::BindBufferRange(
                    gl::UNIFORM_BUFFER,
                    index,
                    buffer,
                    offset as isize,
                    size as isize,
                ));
            }

            for (draw_call, instance_data) in draw_calls {
                gl::call!(gl::BindVertexArray(draw_call.vao));
                // Setup the transform vertex attribute
                let transforms = bytemuck::cast_slice(&instance_data.transforms);
                let (transforms_buffer, transforms_offset) =
                    self.temp_buffer.allocate_buffer(transforms);
                gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, transforms_buffer));
                for i in 0..4 {
                    let attrib_location = model_transform_attrib_locations[i];
                    let offset = transforms_offset + mem::size_of::<Vec4>() * i;
                    gl::call!(gl::EnableVertexAttribArray(attrib_location));
                    gl::call!(gl::VertexAttribPointer(
                        attrib_location,
                        4,
                        gl::FLOAT,
                        gl::FALSE,
                        mem::size_of::<Mat4>() as i32,
                        ptr::null::<c_void>().add(offset)
                    ));
                    gl::call!(gl::VertexAttribDivisor(attrib_location, 1));
                }
                // Setup the texture coordinate transform vertex attribute
                let tx_transforms = bytemuck::cast_slice(&instance_data.texcoord_transforms);
                let (tx_transforms_buffer, tx_transforms_offset) =
                    self.temp_buffer.allocate_buffer(tx_transforms);
                gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, tx_transforms_buffer));
                for i in 0..4 {
                    let attrib_location = texcoord_transform_attrib_locations[i];
                    let offset = tx_transforms_offset + mem::size_of::<Vec4>() * i;
                    gl::call!(gl::EnableVertexAttribArray(attrib_location));
                    gl::call!(gl::VertexAttribPointer(
                        attrib_location,
                        4,
                        gl::FLOAT,
                        gl::FALSE,
                        mem::size_of::<Mat4>() as i32,
                        ptr::null::<c_void>().add(offset)
                    ));
                    gl::call!(gl::VertexAttribDivisor(attrib_location, 1));
                }
                // Set color vertex attribute default value
                if let Some(location) = draw_call.disabled_all_ones_vertex_attribute {
                    gl::call!(gl::VertexAttrib4f(location, 1.0, 1.0, 1.0, 1.0));
                }
                // Set the front face
                gl::call!(gl::FrontFace(draw_call.front_face));
                // Bind the index buffer
                gl::call!(gl::BindBuffer(
                    gl::ELEMENT_ARRAY_BUFFER,
                    draw_call.index_buffer
                ));
                gl::call!(gl::DrawElementsInstanced(
                    draw_call.mode,
                    draw_call.index_count,
                    draw_call.index_type,
                    ptr::null::<c_void>().add(draw_call.index_byte_offset),
                    instance_data.count
                ));
            }
        }
    }

    pub fn clear(&mut self) {
        for draw_calls in self.draws.values_mut() {
            for instance_data in draw_calls.values_mut() {
                instance_data.transforms.clear();
                instance_data.texcoord_transforms.clear();
                instance_data.count = 0;
            }
        }
        self.temp_buffer.clear();
        for i in 0..self.lights_count {
            self.lights_ubo.color_and_kind[i].w = 0.0;
        }
        self.lights_count = 0;
    }
}
