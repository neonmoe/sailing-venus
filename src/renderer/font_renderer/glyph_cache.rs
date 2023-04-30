use std::{collections::HashMap, ffi::c_void};

use crate::renderer::gl;
use fontdue::{
    layout::{GlyphPosition, GlyphRasterConfig},
    Font,
};
use glam::Vec4;

pub struct GlyphCache {
    cache: HashMap<GlyphRasterConfig, (u32, u32, u32, u32)>,
    cursor: (u32, u32),
    width: u32,
    height: u32,
    texture: u32,
    max_height_this_row: u32,
}

impl GlyphCache {
    pub fn new(texture: u32) -> GlyphCache {
        let (width, height) = (2048, 2048);
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for _ in 0..width * height {
            pixels.push(0xFFu8);
            pixels.push(0u8);
            pixels.push(0xFFu8);
            pixels.push(0u8);
        }
        gl::call!(gl::BindTexture(gl::TEXTURE_2D, texture));
        gl::call!(gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as i32,
            width as i32,
            height as i32,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            pixels.as_ptr() as *const c_void,
        ));
        GlyphCache {
            cache: HashMap::new(),
            cursor: (0, 0),
            width,
            height,
            texture,
            max_height_this_row: 0,
        }
    }

    pub fn get_texcoord_transform(&mut self, glyph: &GlyphPosition<()>, fonts: &[Font]) -> Vec4 {
        let (x, y, w, h) = if let Some(cached) = self.cache.get(&glyph.key) {
            *cached
        } else {
            let (x, y, w, h) = self.reserve(glyph.width as u32, glyph.height as u32);
            let (_, pixels) = fonts[glyph.font_index].rasterize_config(glyph.key);
            let mut rgba_pixels = Vec::with_capacity(pixels.len() * 4);
            for pixel in pixels {
                rgba_pixels.push(0xFF);
                rgba_pixels.push(0xFF);
                rgba_pixels.push(0xFF);
                rgba_pixels.push(pixel);
            }
            gl::call!(gl::BindTexture(gl::TEXTURE_2D, self.texture));
            gl::call!(gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                x as i32,
                y as i32,
                w as i32,
                h as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba_pixels.as_ptr() as *const c_void,
            ));
            self.cache.insert(glyph.key, (x, y, w, h));
            (x, y, w, h)
        };

        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        let (tw, th) = (self.width as f32, self.height as f32);
        Vec4::new(x / tw, y / th, w / tw, h / th)
    }

    fn reserve(&mut self, width: u32, height: u32) -> (u32, u32, u32, u32) {
        let result = (self.cursor.0, self.cursor.1, width, height);
        assert!(result.0 + result.2 <= self.width);
        assert!(result.1 + result.3 <= self.height);
        if self.cursor.0 + width < self.width {
            self.cursor.0 += width + 1;
        } else {
            self.cursor.0 = 0;
            self.cursor.1 += self.max_height_this_row + 1;
            self.max_height_this_row = 0;
        }
        self.max_height_this_row = self.max_height_this_row.max(height);
        result
    }
}
