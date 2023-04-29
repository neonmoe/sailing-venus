use crate::renderer::bumpalloc_buffer::BumpAllocatedBuffer;
use crate::renderer::draw_calls::{DrawCall, Uniforms};
use crate::renderer::gltf::MAX_LIGHTS;
use crate::renderer::{gl, gltf, FORWARD};
use bytemuck::Zeroable;
use glam::{Mat4, Quat, Vec3, Vec4};
use image::imageops::FilterType;
use image::DynamicImage;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_4;
use std::ffi::c_void;
use std::ptr;
use tinyjson::JsonValue;

// TODO: load_glb

#[track_caller]
pub fn load_gltf(gltf: &str, resources: &[(&str, &[u8])]) -> gltf::Gltf {
    let gltf: JsonValue = gltf.parse().unwrap();
    let gltf = gltf.get::<HashMap<_, _>>().unwrap();

    if let Some(required_exts) = gltf.get("extensionsRequired") {
        let required_exts = required_exts.get::<Vec<_>>().unwrap();
        let unsupported_exts = required_exts
            .iter()
            .flat_map(
                |ext_name: &JsonValue| match ext_name.get::<String>().unwrap().as_str() {
                    "KHR_lights_punctual" => None,
                    ext_name => Some(ext_name),
                },
            )
            .collect::<Vec<_>>();
        if !unsupported_exts.is_empty() {
            panic!("gltf requires unsupported extensions: {unsupported_exts:?}");
        }
    }

    // TODO: Measure how much of the buffers is unused after load (i.e. used by textures and index buffers)
    let buffers_json = gltf["buffers"].get::<Vec<_>>().unwrap();
    let mut gl_buffers = vec![0; buffers_json.len()];
    let mut buffer_slices = Vec::with_capacity(buffers_json.len());
    gl::call!(gl::GenBuffers(
        gl_buffers.len() as i32,
        gl_buffers.as_mut_ptr()
    ));
    for (i, buffer) in buffers_json.into_iter().enumerate() {
        let gl_buffer = gl_buffers[i];
        let buffer: &HashMap<_, _> = buffer.get().unwrap();
        let buffer_resource_name = if i != 0 || buffer.contains_key("uri") {
            buffer["uri"].get::<String>().unwrap()
        } else {
            "" // The BIN buffer of GLBs
        };
        let mut buffer_data = None;
        for (resource_name, data) in resources {
            if *resource_name == buffer_resource_name {
                buffer_data = Some(data);
            }
        }
        let Some(buffer_data) = buffer_data else {
            panic!("could not find buffer with uri \"{buffer_resource_name}\"");
        };
        let byte_length = take_usize(&buffer["byteLength"]);
        assert_eq!(byte_length, buffer_data.len());
        gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, gl_buffer));
        gl::call!(gl::BufferData(
            gl::ARRAY_BUFFER,
            byte_length as isize,
            buffer_data.as_ptr() as *const c_void,
            gl::STATIC_READ,
        ));
        buffer_slices.push(*buffer_data);
    }
    gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, 0));
    let get_buffer_slice = |buffer: usize, offset: usize, length: usize| {
        &buffer_slices[buffer][offset..offset + length]
    };

    let scenes_json = gltf["scenes"].get::<Vec<_>>().unwrap();
    let mut scenes = Vec::with_capacity(scenes_json.len());
    for scene in scenes_json {
        let node_indices = scene["nodes"].get::<Vec<_>>().unwrap();
        let node_indices = node_indices.iter().map(take_usize).collect::<Vec<_>>();
        scenes.push(gltf::Scene { node_indices });
    }
    let scene = take_usize(&gltf["scene"]);

    let nodes_json = gltf["nodes"].get::<Vec<_>>().unwrap();
    let mut nodes = Vec::with_capacity(nodes_json.len());
    for node in nodes_json {
        let node: &HashMap<_, _> = node.get().unwrap();
        let child_node_indices = if let Some(children) = node.get("children") {
            let children = children.get::<Vec<_>>().unwrap();
            children.iter().map(take_usize).collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mesh_index = node.get("mesh").map(take_usize);
        let transform = if let Some(matrix_values) = node.get("matrix") {
            let matrix_values = matrix_values.get::<Vec<_>>().unwrap();
            let mut matrix: [f32; 16] = [0.0; 16];
            assert_eq!(16, matrix_values.len());
            for (i, value) in matrix_values.into_iter().enumerate() {
                matrix[i] = take_f32(&value);
            }
            Mat4::from_cols_slice(&matrix)
        } else {
            let translation = node.get("translation").map(take_vec3).unwrap_or(Vec3::ZERO);
            let scale = node.get("scale").map(take_vec3).unwrap_or(Vec3::ONE);
            let rotation = node
                .get("rotation")
                .map(take_quat)
                .unwrap_or(Quat::IDENTITY);
            Mat4::from_scale_rotation_translation(scale, rotation, translation)
        };
        nodes.push(gltf::Node {
            name: node["name"].get::<String>().unwrap().clone(),
            mesh_index,
            child_node_indices,
            transform,
            original_transform: transform,
        });
    }

    let accessors_json = gltf["accessors"].get::<Vec<_>>().unwrap();
    let buffer_views_json = gltf["bufferViews"].get::<Vec<_>>().unwrap();
    let unpack_accessor = |accessor: usize| {
        let accessor = accessors_json[accessor].get::<HashMap<_, _>>().unwrap();
        let buffer_view = buffer_views_json[take_usize(&accessor["bufferView"])]
            .get::<HashMap<_, _>>()
            .unwrap();

        let buffer = take_usize(&buffer_view["buffer"]);
        let byte_offset = accessor.get("byteOffset").map(take_usize).unwrap_or(0)
            + buffer_view.get("byteOffset").map(take_usize).unwrap_or(0);
        let count = take_usize(&accessor["count"]) as gl::types::GLint;
        assert!(
            !buffer_view.contains_key("byteStride"),
            "byteStride is not supported for attributes"
        );
        let size = match accessor["type"].get::<String>().unwrap().as_ref() {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            type_ => panic!("unexpected vertex attribute accessor type \"{type_}\""),
        };
        let type_ = take_usize(&accessor["componentType"]) as gl::types::GLuint;
        let normalized = accessor
            .get("normalized")
            .map(|v| *v.get::<bool>().unwrap())
            .unwrap_or(false);

        (buffer, byte_offset, count, size, type_, normalized)
    };

    let meshes_json = gltf["meshes"].get::<Vec<_>>().unwrap();
    let primitive_count = meshes_json
        .iter()
        .flat_map(|mesh| mesh["primitives"].get::<Vec<_>>())
        .count();
    let mut gl_vaos = vec![0; primitive_count];
    gl::call!(gl::GenVertexArrays(
        gl_vaos.len() as i32,
        gl_vaos.as_mut_ptr()
    ));
    let mut index_buffer_allocator =
        BumpAllocatedBuffer::new(gl::ELEMENT_ARRAY_BUFFER, gl::DYNAMIC_DRAW);
    gl_buffers.push(index_buffer_allocator.get_buffer(true));
    let mut primitives = Vec::with_capacity(primitive_count);
    let mut meshes = Vec::with_capacity(meshes_json.len());
    for mesh in meshes_json {
        let primitives_json = mesh["primitives"].get::<Vec<_>>().unwrap();
        let mut primitive_indices = Vec::with_capacity(primitives_json.len());
        for primitive_json in primitives_json {
            let primitive_json = primitive_json.get::<HashMap<_, _>>().unwrap();

            let primitive_index = primitives.len();
            let material_index = take_usize(&primitive_json["material"]);
            let mode = primitive_json.get("mode").map(take_usize).unwrap_or(4) as gl::types::GLuint;
            let vao = gl_vaos[primitive_index];
            let mut disabled_all_ones_vertex_attribute = Some(gltf::ATTR_LOC_COLOR_0);
            let attribute_accessors = primitive_json["attributes"].get::<HashMap<_, _>>().unwrap();
            gl::call!(gl::BindVertexArray(vao));
            for (attr_name, accessor) in attribute_accessors {
                let accessor = take_usize(accessor);
                let location = match attr_name.as_ref() {
                    "POSITION" => gltf::ATTR_LOC_POSITION,
                    "NORMAL" => gltf::ATTR_LOC_NORMAL,
                    "TANGENT" => gltf::ATTR_LOC_TANGENT,
                    "TEXCOORD_0" => gltf::ATTR_LOC_TEXCOORD_0,
                    "TEXCOORD_1" => gltf::ATTR_LOC_TEXCOORD_1,
                    "COLOR_0" => gltf::ATTR_LOC_COLOR_0,
                    attr => panic!("unsupported attribute semantic \"{attr}\""),
                };
                let (buffer, offset, _, size, type_, normalized) = unpack_accessor(accessor);
                gl::call!(gl::EnableVertexAttribArray(location));
                gl::call!(gl::BindBuffer(gl::ARRAY_BUFFER, gl_buffers[buffer]));
                gl::call!(gl::VertexAttribPointer(
                    location,
                    size,
                    type_,
                    if normalized { gl::TRUE } else { gl::FALSE },
                    0,
                    ptr::null::<c_void>().add(offset),
                ));
                if location == gltf::ATTR_LOC_COLOR_0 {
                    disabled_all_ones_vertex_attribute = None;
                }
            }

            let indices_accessor = take_usize(&primitive_json["indices"]);
            let (index_buffer, index_byte_offset, index_count, size, index_type, _) =
                unpack_accessor(indices_accessor);
            let index_type_byte_size = match index_type {
                gl::UNSIGNED_BYTE => 1,
                gl::UNSIGNED_SHORT => 2,
                gl::UNSIGNED_INT => 4,
                type_ => panic!("invalid index buffer type {type_}"),
            };
            let index_byte_length = (index_count * size * index_type_byte_size) as usize;
            let index_buffer = get_buffer_slice(index_buffer, index_byte_offset, index_byte_length);
            let (index_buffer, index_byte_offset) =
                index_buffer_allocator.allocate_buffer(index_buffer);

            primitives.push(gltf::Primitive {
                material_index,
                draw_call: DrawCall {
                    mode,
                    vao,
                    index_type,
                    index_buffer,
                    index_byte_offset,
                    index_count,
                    disabled_all_ones_vertex_attribute,
                    front_face: gl::CCW,
                },
            });
            primitive_indices.push(primitive_index);
        }
        meshes.push(gltf::Mesh { primitive_indices });
    }

    let materials_json = gltf["materials"].get::<Vec<_>>().unwrap();
    let textures_json = gltf["textures"].get::<Vec<_>>().unwrap();
    let images_json = gltf["images"].get::<Vec<_>>().unwrap();
    let mut is_srgb = vec![None; images_json.len()];
    for material in materials_json {
        let material = material.get::<HashMap<_, _>>().unwrap();
        let pbr_image = |name: &str| {
            let pbr = material
                .get("pbrMetallicRoughness")?
                .get::<HashMap<_, _>>()
                .unwrap();
            let texture = take_usize(&pbr.get(name)?["index"]);
            Some(take_usize(&textures_json[texture]["source"]))
        };
        let additional_image = |name: &str| {
            let texture = take_usize(&material.get(name)?["index"]);
            Some(take_usize(&textures_json[texture]["source"]))
        };
        let set_srgb_status = |is_srgb: &mut [Option<bool>], index: usize, expected: bool| {
            assert!(
                is_srgb[index] != Some(!expected),
                "images[{}] is used both as srgb and not",
                index,
            );
            is_srgb[index] = Some(expected);
        };
        if let Some(image) = pbr_image("baseColorTexture") {
            set_srgb_status(&mut is_srgb, image, true);
        }
        if let Some(image) = pbr_image("metallicRoughnessTexture") {
            set_srgb_status(&mut is_srgb, image, false);
        }
        if let Some(image) = additional_image("normalTexture") {
            set_srgb_status(&mut is_srgb, image, false);
        }
        if let Some(image) = additional_image("occlusionTexture") {
            set_srgb_status(&mut is_srgb, image, false);
        }
        if let Some(image) = additional_image("emissiveTexture") {
            set_srgb_status(&mut is_srgb, image, true);
        }
    }

    let mut gl_textures = vec![0; images_json.len() + 3];
    gl::call!(gl::GenTextures(
        gl_textures.len() as i32,
        gl_textures.as_mut_ptr()
    ));
    let white_tex = gl_textures[gl_textures.len() - 1];
    let normal_tex = gl_textures[gl_textures.len() - 2];
    let black_tex = gl_textures[gl_textures.len() - 3];
    let make_pixel_tex = |tex: u32, color: [u8; 3]| {
        let target = gl::TEXTURE_2D;
        let ifmt = gl::RGB as i32;
        let fmt = gl::RGB;
        let type_ = gl::UNSIGNED_BYTE;
        let pixels = color.as_ptr() as *const c_void;
        gl::call!(gl::BindTexture(target, tex));
        gl::call!(gl::TexImage2D(target, 0, ifmt, 1, 1, 0, fmt, type_, pixels));
    };
    make_pixel_tex(white_tex, [0xFF, 0xFF, 0xFF]);
    make_pixel_tex(normal_tex, [0x7F, 0x7F, 0xFF]);
    make_pixel_tex(black_tex, [0, 0, 0]);
    for (i, image) in images_json.into_iter().enumerate() {
        let Some(is_srgb) = is_srgb[i] else {
            continue; // Not used by any material.
        };

        let image = image.get::<HashMap<_, _>>().unwrap();
        let image_data = if let Some(uri) = image.get("uri") {
            let uri = uri.get::<String>().unwrap().as_str();
            match resources
                .iter()
                .find(|(name, _)| *name == uri)
                .map(|(_, data)| *data)
            {
                Some(data) => data,
                None => panic!("the uri of image {i} ({uri}) is not included in resources"),
            }
        } else {
            let buffer_view = image["bufferView"].get::<HashMap<_, _>>().unwrap();
            let buffer = take_usize(&buffer_view["buffer"]);
            let offset = buffer_view.get("byteOffset").map(take_usize).unwrap_or(0);
            let length = take_usize(&buffer_view["byteLength"]);
            assert!(
                !buffer_view.contains_key("byteStride"),
                "byteStride is not supported for image data"
            );
            get_buffer_slice(buffer, offset, length)
        };

        let mut parsed_image = image::load_from_memory(image_data).unwrap();
        let (format, type_, bpp) = match parsed_image {
            DynamicImage::ImageRgb8(_) => (gl::RGB, gl::UNSIGNED_BYTE, 3),
            DynamicImage::ImageRgba8(_) => (gl::RGBA, gl::UNSIGNED_BYTE, 4),
            DynamicImage::ImageRgb16(_) => (gl::RGB, gl::UNSIGNED_SHORT, 6),
            DynamicImage::ImageRgba16(_) => (gl::RGBA, gl::UNSIGNED_SHORT, 8),
            img => panic!("image {img:?} is of an unsupported format"),
        };
        let internal_format = match (is_srgb, format) {
            (true, gl::RGBA) => gl::SRGB8_ALPHA8,
            (true, gl::RGB) => gl::SRGB8,
            (false, format) => format,
            _ => unreachable!(),
        };
        gl::call!(gl::BindTexture(gl::TEXTURE_2D, gl_textures[i]));
        let size = parsed_image.width().min(parsed_image.height());
        let mip_levels = (size as f32).log2().floor() as i32 + 1;
        for mip_level in 0..mip_levels {
            let (width, height, data) = (
                parsed_image.width() as i32,
                parsed_image.height() as i32,
                parsed_image.as_bytes(),
            );
            assert_eq!(width * height * bpp, data.len() as i32);
            gl::call!(gl::TexImage2D(
                gl::TEXTURE_2D,
                mip_level,
                internal_format as i32,
                width,
                height,
                0,
                format,
                type_,
                data.as_ptr() as *const c_void,
            ));
            if mip_level < mip_levels - 1 {
                parsed_image = parsed_image.resize_exact(
                    width as u32 / 2,
                    height as u32 / 2,
                    if is_srgb {
                        FilterType::CatmullRom
                    } else {
                        FilterType::Triangle
                    },
                );
            }
        }
    }

    let samplers_json_fallback = Vec::with_capacity(0);
    let samplers_json = gltf
        .get("samplers")
        .map(|v| v.get::<Vec<_>>().unwrap())
        .unwrap_or(&samplers_json_fallback);
    let mut gl_samplers = vec![0; samplers_json.len() + 1];
    gl::call!(gl::GenSamplers(
        gl_samplers.len() as i32,
        gl_samplers.as_mut_ptr()
    ));
    let default_sampler = gl_samplers[gl_samplers.len() - 1];
    gl::call!(gl::SamplerParameteri(
        default_sampler,
        gl::TEXTURE_MAG_FILTER,
        gl::LINEAR as i32,
    ));
    gl::call!(gl::SamplerParameteri(
        default_sampler,
        gl::TEXTURE_MIN_FILTER,
        gl::LINEAR_MIPMAP_LINEAR as i32,
    ));
    gl::call!(gl::SamplerParameteri(
        default_sampler,
        gl::TEXTURE_WRAP_S,
        gl::REPEAT as i32,
    ));
    gl::call!(gl::SamplerParameteri(
        default_sampler,
        gl::TEXTURE_WRAP_T,
        gl::REPEAT as i32,
    ));
    for (i, sampler) in samplers_json.into_iter().enumerate() {
        let sampler = sampler.get::<HashMap<_, _>>().unwrap();
        gl::call!(gl::SamplerParameteri(
            gl_samplers[i],
            gl::TEXTURE_MAG_FILTER,
            sampler
                .get("magFilter")
                .map(take_usize)
                .unwrap_or(gl::LINEAR as usize) as i32,
        ));
        gl::call!(gl::SamplerParameteri(
            gl_samplers[i],
            gl::TEXTURE_MIN_FILTER,
            sampler
                .get("minFilter")
                .map(take_usize)
                .unwrap_or(gl::LINEAR_MIPMAP_LINEAR as usize) as i32,
        ));
        gl::call!(gl::SamplerParameteri(
            gl_samplers[i],
            gl::TEXTURE_WRAP_S,
            sampler.get("wrapS").map(take_usize).unwrap_or(10497) as i32,
        ));
        gl::call!(gl::SamplerParameteri(
            gl_samplers[i],
            gl::TEXTURE_WRAP_T,
            sampler.get("wrapT").map(take_usize).unwrap_or(10497) as i32,
        ));
    }

    let mut uniform_buffer_allocator =
        BumpAllocatedBuffer::new(gl::UNIFORM_BUFFER, gl::DYNAMIC_DRAW);
    gl_buffers.push(uniform_buffer_allocator.get_buffer(true));

    // KHR_lights_punctual extension:
    let lights_json_fallback = Vec::with_capacity(0);
    let lights_json = gltf
        .get("extensions")
        .and_then(|v| v.get::<HashMap<_, _>>().unwrap().get("KHR_lights_punctual"))
        .and_then(|v| v.get::<HashMap<_, _>>().unwrap().get("lights"))
        .map(|v| v.get::<Vec<_>>().unwrap())
        .unwrap_or(&lights_json_fallback);
    let mut lights = gltf::UniformBlockLights::zeroed();
    let mut light_node_index = 0;
    for (node_index, node) in nodes_json.into_iter().enumerate() {
        if let Some(khr_lights_punctual) = node
            .get::<HashMap<_, _>>()
            .unwrap()
            .get("extensions")
            .map(|extensions| extensions.get::<HashMap<_, _>>().unwrap())
            .map(|extensions| extensions.get("KHR_lights_punctual"))
            .flatten()
        {
            if light_node_index >= MAX_LIGHTS {
                panic!("this gltf renderer only supports a maximum of {MAX_LIGHTS} lights");
            }

            let light_index = take_usize(&khr_lights_punctual["light"]);
            let light = lights_json[light_index].get::<HashMap<_, _>>().unwrap();
            let color = light.get("color").map(take_vec3).unwrap_or(Vec3::ONE);
            let intensity = light.get("intensity").map(take_f32).unwrap_or(1.0);
            let kind = match light["type"].get::<String>().unwrap().as_str() {
                "directional" => 1.0,
                "point" => 2.0,
                "spot" => 3.0,
                kind => panic!("light has a non-standard type '{kind}'"),
            };
            let transform = nodes[node_index].transform;
            let inner_angle = light.get("innerConeAngle").map(take_f32).unwrap_or(0.0);
            let outer_angle = light
                .get("outerConeAngle")
                .map(take_f32)
                .unwrap_or(FRAC_PI_4);
            // https://github.com/KhronosGroup/glTF/blob/main/extensions/2.0/Khronos/KHR_lights_punctual/README.md#inner-and-outer-cone-angles
            let light_angle_scale = 1.0 / 0.001f32.max(inner_angle.cos() - outer_angle.cos());
            let light_angle_offset = -outer_angle.cos() * light_angle_scale;

            let i = light_node_index;
            lights.color_and_kind[i] = Vec4::from((color, kind));
            lights.intensity_params[i] =
                Vec4::new(intensity, light_angle_scale, light_angle_offset, 0.0);
            lights.position[i] = transform * Vec4::new(0.0, 0.0, 0.0, 1.0);
            lights.direction[i] = transform * Vec4::from((FORWARD, 0.0));
            light_node_index += 1;
        }
    }
    let lights_uniform_block = {
        let lights_data = [lights];
        let lights_data = bytemuck::cast_slice(&lights_data);
        let (ubo, ubo_offset) = uniform_buffer_allocator.allocate_buffer(lights_data);
        let ubo_size = lights_data.len();
        (gltf::UNIFORM_BLOCK_LIGHTS, ubo, ubo_offset, ubo_size)
    };

    let materials_json = gltf["materials"].get::<Vec<_>>().unwrap();
    let mut materials = Vec::with_capacity(materials_json.len());
    for material in materials_json {
        let unpack_texture_info = |texture_info: &JsonValue| {
            let texture_info = texture_info.get::<HashMap<_, _>>().unwrap();
            // TODO: Support TEXCOORD_1
            assert!(matches!(
                texture_info.get("texCoord").map(take_usize),
                None | Some(0)
            ));
            let texture = &textures_json[take_usize(&texture_info["index"])];
            let texture = texture.get::<HashMap<_, _>>().unwrap();
            let sampler = texture
                .get("sampler")
                .map(take_usize)
                .unwrap_or(gl_samplers.len() - 1);
            let source = take_usize(&texture["source"]);
            (gl_textures[source], gl_samplers[sampler])
        };

        let material = material.get::<HashMap<_, _>>().unwrap();
        let mut material_buffer = gltf::UniformBlockMaterial {
            base_color_factor: Vec4::splat(1.0),
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            emissive_factor: Vec4::splat(0.0),
        };

        let mut textures = [None; 5];
        if let Some(pbr) = material.get("pbrMetallicRoughness") {
            let pbr = pbr.get::<HashMap<_, _>>().unwrap();
            if let Some(texture_info) = pbr.get("baseColorTexture") {
                let (texture, sampler) = unpack_texture_info(texture_info);
                textures[0] = Some((gltf::TEX_UNIT_BASE_COLOR, texture, sampler));
            } else {
                textures[0] = Some((gltf::TEX_UNIT_BASE_COLOR, white_tex, default_sampler));
            }
            if let Some(texture_info) = pbr.get("metallicRoughnessTexture") {
                let (texture, sampler) = unpack_texture_info(texture_info);
                textures[1] = Some((gltf::TEX_UNIT_METALLIC_ROUGHNESS, texture, sampler));
            } else {
                textures[1] = Some((
                    gltf::TEX_UNIT_METALLIC_ROUGHNESS,
                    white_tex,
                    default_sampler,
                ));
            }
            if let Some(factor) = pbr.get("baseColorFactor") {
                let factor = factor.get::<Vec<_>>().unwrap();
                let x = take_f32(&factor[0]);
                let y = take_f32(&factor[1]);
                let z = take_f32(&factor[2]);
                let w = take_f32(&factor[3]);
                material_buffer.base_color_factor = Vec4::new(x, y, z, w);
            }
            if let Some(factor) = pbr.get("metallicFactor") {
                material_buffer.metallic_factor = take_f32(&factor);
            }
            if let Some(factor) = pbr.get("roughnessFactor") {
                material_buffer.roughness_factor = take_f32(&factor);
            }
        }
        if let Some(texture_info) = material.get("normalTexture") {
            let (texture, sampler) = unpack_texture_info(texture_info);
            textures[2] = Some((gltf::TEX_UNIT_NORMAL, texture, sampler));
            let texture_info = texture_info.get::<HashMap<_, _>>().unwrap();
            if let Some(factor) = texture_info.get("scale") {
                material_buffer.normal_scale = take_f32(&factor);
            }
        } else {
            textures[2] = Some((gltf::TEX_UNIT_NORMAL, normal_tex, default_sampler));
        }
        if let Some(texture_info) = material.get("occlusionTexture") {
            let (texture, sampler) = unpack_texture_info(texture_info);
            textures[3] = Some((gltf::TEX_UNIT_OCCLUSION, texture, sampler));
            let texture_info = texture_info.get::<HashMap<_, _>>().unwrap();
            if let Some(factor) = texture_info.get("strength") {
                material_buffer.occlusion_strength = take_f32(&factor);
            }
        } else {
            textures[3] = Some((gltf::TEX_UNIT_OCCLUSION, white_tex, default_sampler));
        }
        if let Some(texture_info) = material.get("emissiveTexture") {
            let (texture, sampler) = unpack_texture_info(texture_info);
            textures[4] = Some((gltf::TEX_UNIT_EMISSIVE, texture, sampler));
        } else {
            textures[4] = Some((gltf::TEX_UNIT_EMISSIVE, black_tex, default_sampler));
        }
        if let Some(factor) = material.get("emissiveFactor") {
            let factor = factor.get::<Vec<_>>().unwrap();
            let x = take_f32(&factor[0]);
            let y = take_f32(&factor[1]);
            let z = take_f32(&factor[2]);
            material_buffer.emissive_factor = Vec4::new(x, y, z, 1.0);
        }

        let material_data = [material_buffer];
        let material_data = bytemuck::cast_slice(&material_data);
        let (ubo, ubo_offset) = uniform_buffer_allocator.allocate_buffer(material_data);
        let ubo_size = material_data.len();
        let ubos = [
            Some((gltf::UNIFORM_BLOCK_MATERIAL, ubo, ubo_offset, ubo_size)),
            Some(lights_uniform_block),
        ];

        materials.push(gltf::Material {
            name: material["name"].get::<String>().unwrap().clone(),
            uniforms: Uniforms { textures, ubos },
        });
    }

    let animations_json_fallback = Vec::with_capacity(0);
    let animations_json = gltf
        .get("animations")
        .map(|v| v.get::<Vec<_>>().unwrap())
        .unwrap_or(&animations_json_fallback);
    let mut animations = Vec::with_capacity(animations_json.len());
    for animation in animations_json {
        let name = animation["name"].get::<String>().unwrap().to_string();
        let mut start = f32::INFINITY;
        let mut end = f32::NEG_INFINITY;
        let mut nodes_animations = vec![Vec::new(); nodes.len()];
        let samplers = animation["samplers"].get::<Vec<_>>().unwrap();
        for channel in animation["channels"].get::<Vec<_>>().unwrap() {
            let get_accessor_slice = |accessor: usize, bpc: usize| {
                let (buffer, offset, count, ..) = unpack_accessor(accessor);
                let length = count as usize * bpc;
                get_buffer_slice(buffer, offset, length)
            };

            let sampler = &samplers[take_usize(&channel["sampler"])];
            let node = take_usize(&channel["target"]["node"]);
            let path = channel["target"]["path"].get::<String>().unwrap().as_str();
            let input_accessor = take_usize(&sampler["input"]);
            let input = get_accessor_slice(input_accessor, 4);
            let output_bpc = if path == "rotation" { 16 } else { 12 };
            let output = get_accessor_slice(take_usize(&sampler["output"]), output_bpc);
            let timestamps = bytemuck::pod_collect_to_vec(input);
            start = start.min(timestamps[0]);
            end = end.max(timestamps[timestamps.len() - 1]);
            let keyframes = match path {
                "translation" => gltf::Keyframes::Translation(bytemuck::pod_collect_to_vec(output)),
                "rotation" => gltf::Keyframes::Rotation(bytemuck::pod_collect_to_vec(output)),
                "scale" => gltf::Keyframes::Scale(bytemuck::pod_collect_to_vec(output)),
                target => panic!("unsupported animation target '{target}'"),
            };
            let interpolation = match sampler["interpolation"].get::<String>().unwrap().as_str() {
                "STEP" => gltf::Interpolation::Step,
                "LINEAR" => gltf::Interpolation::Linear,
                "CUBICSPLINE" => gltf::Interpolation::CubicSpline,
                interp => panic!("invalid interpolation '{interp}'"),
            };
            nodes_animations[node].push(gltf::NodeAnimation {
                timestamps,
                keyframes,
                interpolation,
            });
        }
        animations.push(gltf::Animation {
            name,
            nodes_animations,
            start,
            length: end - start,
        });
    }

    gltf::Gltf {
        scene,
        animations,
        scenes,
        nodes,
        meshes,
        materials,
        primitives,
        gl_vaos,
        gl_buffers,
        gl_textures,
        gl_samplers,
    }
}

/// Return usize if JsonValue is a number, otherwise panic.
fn take_usize(json_value: &JsonValue) -> usize {
    let i: &f64 = json_value.get().unwrap();
    *i as usize
}

/// Return f32 if JsonValue is a number, otherwise panic.
fn take_f32(json_value: &JsonValue) -> f32 {
    let f: &f64 = json_value.get().unwrap();
    *f as f32
}

/// Return Vec3 if JsonValue is an array, otherwise panic.
fn take_vec3(json_value: &JsonValue) -> Vec3 {
    let values: &Vec<JsonValue> = json_value.get().unwrap();
    assert_eq!(3, values.len());
    let x = *values[0].get::<f64>().unwrap() as f32;
    let y = *values[1].get::<f64>().unwrap() as f32;
    let z = *values[2].get::<f64>().unwrap() as f32;
    Vec3::new(x, y, z)
}

/// Return Quat if JsonValue is an array, otherwise panic.
fn take_quat(json_value: &JsonValue) -> Quat {
    let values: &Vec<JsonValue> = json_value.get().unwrap();
    assert_eq!(4, values.len());
    let x = *values[0].get::<f64>().unwrap() as f32;
    let y = *values[1].get::<f64>().unwrap() as f32;
    let z = *values[2].get::<f64>().unwrap() as f32;
    let w = *values[3].get::<f64>().unwrap() as f32;
    Quat::from_xyzw(x, y, z, w)
}
