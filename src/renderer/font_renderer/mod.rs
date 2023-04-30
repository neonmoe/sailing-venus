use crate::renderer::bumpalloc_buffer::BumpAllocatedBuffer;
use crate::renderer::draw_calls::{DrawCall, Uniforms};
use crate::renderer::{gl, gltf, DrawCalls};
use bytemuck::Zeroable;
use fontdue::layout::{
    CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign,
};
use fontdue::{Font, FontSettings};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use std::ffi::c_void;
use std::ptr;

mod glyph_cache;

use glyph_cache::GlyphCache;

type IndexType = u16;
const INDEX_COUNT: i32 = 6;
const INDEX_TYPE: u32 = gl::UNSIGNED_SHORT;

pub struct FontRenderer {
    glyph_uniforms: Uniforms,
    glyph_draw_call: DrawCall,
    glyph_cache: GlyphCache,
    fonts: Vec<Font>,
    layout: Layout,

    gl_vao: u32,
    gl_buffers: [u32; 2],
    gl_textures: [u32; 3],
    gl_sampler: u32,
}

impl FontRenderer {
    pub fn new() -> FontRenderer {
        let mut allocator = BumpAllocatedBuffer::new(gl::ARRAY_BUFFER, gl::DYNAMIC_DRAW);
        let array_buffer = allocator.get_buffer(true);
        let mut index_allocator =
            BumpAllocatedBuffer::new(gl::ELEMENT_ARRAY_BUFFER, gl::DYNAMIC_DRAW);
        let index_buffer = index_allocator.get_buffer(true);

        let position: [f32; 3 * 4] = [
            0.0, 0.0, 0.0, // Bottom-left
            1.0, 0.0, 0.0, // Bottom-right
            0.0, 1.0, 0.0, // Top-left
            1.0, 1.0, 0.0, // Top-right
        ];
        let texcoords: [f32; 2 * 4] = [
            0.0, 1.0, // Bottom-left
            1.0, 1.0, // Bottom-right
            0.0, 0.0, // Top-left
            1.0, 0.0, // Top-right
        ];
        let indices: [IndexType; 6] = [0, 1, 2, 2, 1, 3];
        let (pos_buffer, pos_offset) = allocator.allocate_buffer(bytemuck::cast_slice(&position));
        let (tex_buffer, tex_offset) = allocator.allocate_buffer(bytemuck::cast_slice(&texcoords));
        let (idx_buffer, idx_offset) =
            index_allocator.allocate_buffer(bytemuck::cast_slice(&indices));
        let mut gl_vao = 0;
        gl::call!(gl::GenVertexArrays(1, &mut gl_vao));
        gl::call!(gl::BindVertexArray(gl_vao));
        gl::call!(gl::EnableVertexAttribArray(gltf::ATTR_LOC_POSITION));
        gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, pos_buffer));
        gl::call!(gl::VertexAttribPointer(
            gltf::ATTR_LOC_POSITION,
            3,
            gl::FLOAT,
            gl::FALSE,
            0,
            ptr::null::<c_void>().add(pos_offset),
        ));
        gl::call!(gl::EnableVertexAttribArray(gltf::ATTR_LOC_TEXCOORD_0));
        gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, tex_buffer));
        gl::call!(gl::VertexAttribPointer(
            gltf::ATTR_LOC_TEXCOORD_0,
            2,
            gl::FLOAT,
            gl::FALSE,
            0,
            ptr::null::<c_void>().add(tex_offset),
        ));
        let disabled_all_ones_vertex_attribute = Some(gltf::ATTR_LOC_COLOR_0);
        let glyph_draw_call = DrawCall {
            vao: gl_vao,
            mode: gl::TRIANGLES,
            index_buffer: idx_buffer,
            index_type: INDEX_TYPE,
            index_byte_offset: idx_offset,
            index_count: INDEX_COUNT,
            disabled_all_ones_vertex_attribute,
            front_face: gl::CCW,
        };

        let mut gl_textures = [0; 3];
        let mut gl_sampler = 0;
        gl::call!(gl::GenTextures(
            gl_textures.len() as i32,
            gl_textures.as_mut_ptr()
        ));
        gl::call!(gl::GenSamplers(1, &mut gl_sampler));
        gl::setup_linear_sampler(gl_sampler, false);
        let [glyph_tex, white, normal] = gl_textures;
        gl::write_1px_rgb_texture(glyph_tex, [0x99, 0x33, 0xBB]);
        gl::write_1px_rgb_texture(white, [0xFF, 0xFF, 0xFF]);
        gl::write_1px_rgb_texture(normal, [0x7F, 0x7F, 0xFF]);
        let textures = [
            Some((gltf::TEX_UNIT_BASE_COLOR, glyph_tex, gl_sampler)),
            Some((gltf::TEX_UNIT_METALLIC_ROUGHNESS, white, gl_sampler)),
            Some((gltf::TEX_UNIT_NORMAL, normal, gl_sampler)),
            Some((gltf::TEX_UNIT_OCCLUSION, white, gl_sampler)),
            Some((gltf::TEX_UNIT_EMISSIVE, glyph_tex, gl_sampler)),
        ];
        let material = [gltf::UniformBlockMaterial {
            base_color_factor: Vec4::new(0.0, 0.0, 0.0, 1.0),
            metallic_factor: 0.0,
            roughness_factor: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            emissive_factor: Vec4::new(1.0, 1.0, 1.0, 1.0),
        }];
        let mat_bytes = bytemuck::cast_slice(&material);
        let mat_size = mat_bytes.len();
        let (mat_buf, mat_off) = allocator.allocate_buffer(mat_bytes);
        let lights = [gltf::UniformBlockLights::zeroed()];
        let lgt_bytes = bytemuck::cast_slice(&lights);
        let lgt_size = lgt_bytes.len();
        let (lgt_buf, lgt_off) = allocator.allocate_buffer(lgt_bytes);
        let ubos = [
            Some((gltf::UNIFORM_BLOCK_MATERIAL, mat_buf, mat_off, mat_size)),
            Some((gltf::UNIFORM_BLOCK_LIGHTS, lgt_buf, lgt_off, lgt_size)),
        ];
        let glyph_uniforms = Uniforms { textures, ubos };

        let montserrat =
            Font::from_bytes(
                &include_bytes!(
                    "../../../resources/fonts/montserrat/static/Montserrat-SemiBold.ttf"
                )[..],
                FontSettings::default(),
            )
            .unwrap();
        let layout = Layout::new(CoordinateSystem::PositiveYUp);

        FontRenderer {
            glyph_uniforms,
            glyph_draw_call,
            glyph_cache: GlyphCache::new(glyph_tex),
            fonts: vec![montserrat],
            layout,
            gl_vao,
            gl_buffers: [array_buffer, index_buffer],
            gl_textures,
            gl_sampler,
        }
    }

    pub fn draw_text(
        &mut self,
        draw_calls: &mut DrawCalls,
        text: &str,
        pos: Vec2,
        depth: f32,
        px: f32,
        (h_align, v_align): (HorizontalAlign, VerticalAlign),
    ) {
        self.layout.reset(&LayoutSettings {
            x: pos.x,
            y: pos.y,
            horizontal_align: h_align,
            vertical_align: v_align,
            ..Default::default()
        });
        let style = TextStyle {
            text,
            px,
            font_index: 0,
            user_data: (),
        };
        self.layout.append(&self.fonts, &style);
        for glyph in self.layout.glyphs() {
            let texcoord = self.glyph_cache.get_texcoord_transform(glyph, &self.fonts);
            let texcoord_transform = Mat4::from_scale_rotation_translation(
                Vec3::new(texcoord.z, texcoord.w, 1.0),
                Quat::IDENTITY,
                Vec3::new(texcoord.x, texcoord.y, 0.0),
            );
            let transform = Mat4::from_scale_rotation_translation(
                Vec3::new(glyph.width as f32, glyph.height as f32, 1.0),
                Quat::IDENTITY,
                Vec3::new(glyph.x, glyph.y, depth),
            );
            draw_calls.add(
                &self.glyph_uniforms,
                &self.glyph_draw_call,
                transform,
                texcoord_transform,
            );
        }
    }
}

impl Drop for FontRenderer {
    fn drop(&mut self) {
        gl::call!(gl::DeleteVertexArrays(1, &self.gl_vao));
        gl::call!(gl::DeleteBuffers(
            self.gl_buffers.len() as i32,
            self.gl_buffers.as_ptr()
        ));
        gl::call!(gl::DeleteTextures(
            self.gl_textures.len() as i32,
            self.gl_textures.as_ptr(),
        ));
        gl::call!(gl::DeleteSamplers(1, &self.gl_sampler));
    }
}
