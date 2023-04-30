use crate::renderer::draw_calls::{DrawCall, DrawCalls, Uniforms};
use crate::renderer::gl;
use glam::Mat4;

mod animation;
mod loader;
mod program;

pub use animation::*;
pub use loader::{load_glb, load_gltf};
pub use program::*;

pub struct Gltf {
    pub scene: usize,
    pub animations: Vec<Animation>,
    scenes: Vec<Scene>,
    nodes: Vec<Node>,
    meshes: Vec<Mesh>,
    materials: Vec<Material>,
    primitives: Vec<Primitive>,

    gl_vaos: Vec<gl::types::GLuint>,
    gl_buffers: Vec<gl::types::GLuint>,
    gl_textures: Vec<gl::types::GLuint>,
    gl_samplers: Vec<gl::types::GLuint>,
}

pub struct Scene {
    node_indices: Vec<usize>,
}

pub struct Node {
    pub name: String,
    pub transform: Mat4,
    pub original_transform: Mat4,
    mesh_index: Option<usize>,
    child_node_indices: Vec<usize>,
}

pub struct Mesh {
    primitive_indices: Vec<usize>,
}

pub struct Primitive {
    pub draw_call: DrawCall,
    material_index: usize,
}

pub struct Material {
    pub name: String,
    pub uniforms: Uniforms,
}

impl Gltf {
    pub fn draw(&self, draw_calls: &mut DrawCalls, model_transform: Mat4) {
        self._draw(draw_calls, model_transform, |i| self.nodes[i].transform)
    }

    pub fn copy_lights_from(&mut self, other: &Gltf) {
        // TODO: Why doesn't this work?
        for (material, other) in self.materials.iter_mut().zip(other.materials.iter()) {
            material.uniforms.ubos[1] = other.uniforms.ubos[1].clone();
        }
    }

    pub fn transform_lights(&mut self) {
        // TODO: Hold the original lights somewhere
        // TODO: Update ubo with newly transformed lights
        todo!();
    }

    pub fn draw_animated(
        &self,
        draw_calls: &mut DrawCalls,
        model_transform: Mat4,
        node_transforms: &[NodeTransform],
    ) {
        self._draw(draw_calls, model_transform, |i| {
            node_transforms[i].transform
        })
    }

    #[inline]
    fn _draw<F: Fn(usize) -> Mat4>(
        &self,
        draw_calls: &mut DrawCalls,
        model_transform: Mat4,
        get_transform: F,
    ) {
        let scene = &self.scenes[self.scene];
        let mut node_queue = scene
            .node_indices
            .iter()
            .map(|&i| (model_transform, i))
            .collect::<Vec<_>>();
        while let Some((parent_transform, node_index)) = node_queue.pop() {
            let transform = parent_transform * get_transform(node_index);
            if let Some(mesh_index) = self.nodes[node_index].mesh_index {
                for &primitive_index in &self.meshes[mesh_index].primitive_indices {
                    let primitive = &self.primitives[primitive_index];
                    let uniforms = &self.materials[primitive.material_index].uniforms;
                    let mut draw_call = primitive.draw_call.clone();
                    // glTF spec section 3.7.4:
                    draw_call.front_face = (transform.determinant() > 0.0)
                        .then_some(gl::CCW)
                        .unwrap_or(gl::CW);
                    draw_calls.add(uniforms, &draw_call, transform, Mat4::IDENTITY);
                }
            }
            for &child_index in &self.nodes[node_index].child_node_indices {
                node_queue.push((transform, child_index));
            }
        }
    }
}

impl Drop for Gltf {
    fn drop(&mut self) {
        gl::call!(gl::DeleteVertexArrays(
            self.gl_vaos.len() as i32,
            self.gl_vaos.as_ptr(),
        ));
        gl::call!(gl::DeleteBuffers(
            self.gl_buffers.len() as i32,
            self.gl_buffers.as_ptr(),
        ));
        gl::call!(gl::DeleteTextures(
            self.gl_textures.len() as i32,
            self.gl_textures.as_ptr(),
        ));
        gl::call!(gl::DeleteSamplers(
            self.gl_samplers.len() as i32,
            self.gl_samplers.as_ptr(),
        ));
    }
}
