use std::ffi::c_void;
use std::ptr;

use crate::renderer::gl;

pub struct BumpAllocatedBuffer {
    buffer: gl::types::GLuint,
    target: gl::types::GLenum,
    usage: gl::types::GLenum,
    offset: usize,
    size: usize,
    data_copy: Vec<u8>,
    buffer_leaked: bool,
}

impl BumpAllocatedBuffer {
    pub fn new(target: gl::types::GLenum, usage: gl::types::GLenum) -> BumpAllocatedBuffer {
        let mut buffer = 0;
        gl::call!(gl::GenBuffers(1, &mut buffer));
        BumpAllocatedBuffer {
            buffer,
            target,
            usage,
            offset: 0,
            size: 0,
            data_copy: Vec::new(),
            buffer_leaked: false,
        }
    }

    /// Returns the internal buffer of the bump allocator. If `leak` is true,
    /// the buffer is marked as "leaked" and not deleted when [Self] is dropped.
    pub fn get_buffer(&mut self, leak: bool) -> gl::types::GLuint {
        self.buffer_leaked |= leak;
        self.buffer
    }

    /// Writes the bytes into the backing buffer of this bump allocator, and
    /// returns the buffer object and offset into it, where the bytes were
    /// written.
    pub fn allocate_buffer(&mut self, bytes: &[u8]) -> (gl::types::GLuint, usize) {
        if self.offset + bytes.len() >= self.size {
            let additional = bytes.len() + self.size;
            let original_size = self.size;
            self.size += additional;
            self.data_copy.reserve_exact(additional);
            gl::call!(gl::BindBuffer(self.target, self.buffer));
            gl::call!(gl::BufferData(
                self.target,
                self.size as isize,
                ptr::null(),
                self.usage,
            ));
            gl::call!(gl::BufferSubData(
                self.target,
                0,
                original_size as isize,
                self.data_copy.as_ptr() as *const c_void,
            ));
        }
        let upload_offset = self.offset;
        gl::call!(gl::BindBuffer(self.target, self.buffer));
        gl::call!(gl::BufferSubData(
            self.target,
            upload_offset as isize,
            bytes.len() as isize,
            bytes.as_ptr() as *const c_void,
        ));
        self.data_copy.extend_from_slice(bytes);
        self.offset += bytes.len();
        (self.buffer, upload_offset)
    }

    pub fn clear(&mut self) {
        self.offset = 0;
        self.data_copy.clear();
    }
}

impl Drop for BumpAllocatedBuffer {
    fn drop(&mut self) {
        if !self.buffer_leaked {
            gl::call!(gl::DeleteBuffers(1, &self.buffer));
        }
    }
}
