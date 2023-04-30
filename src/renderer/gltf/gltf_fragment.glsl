#version 300 es
precision highp float;
precision highp int;

#define PI 3.14159265
#define MAX_LIGHTS 32
#define VIEW_VECTOR vec3(0.0, 0.0, 1.0)

out vec4 FRAG_COLOR;

in vec3 view_pos;
in vec3 vertex_color;
in vec3 vertex_normal;
in vec4 vertex_tangent;
in vec2 tex_coords;

uniform mat4 view_from_world;
uniform sampler2D base_color_tex;
uniform sampler2D metallic_roughness_tex;
uniform sampler2D normal_tex;
uniform sampler2D occlusion_tex;
uniform sampler2D emissive_tex;
layout(std140) uniform Material {
  vec4 base_color_factor;
  // x: metallic factor, y: roughness factor, z: normal scale, w: occlusion
  // strength
  vec4 material_params;
  vec4 emissive_factor;
};
layout(std140) uniform Lights {
  // w: 0.0 as the null terminator, 1.0: directional, 2.0: point, 3.0: spot,
  // xyz: rgb
  vec4 light_color_and_kind[MAX_LIGHTS];
  // x: intensity, y: angle scale, z: angle offset
  vec4 light_intensity_params[MAX_LIGHTS];
  vec4 light_position[MAX_LIGHTS];
  vec4 light_direction[MAX_LIGHTS];
};

vec3 aces_filmic(vec3 x) {
  float a = 2.51;
  float b = 0.03;
  float c = 2.43;
  float d = 0.59;
  float e = 0.14;
  return clamp(x * (a * x + b) / (x * (c * x + d) + e), vec3(0), vec3(1));
}

vec3 diffuse_brdf(vec3 color) { return color / PI; }

// There's no basis or source for any of this, except the idea of dotting the
// reflected direction with the view direction. I just want some speculars.
float specular(vec3 reflected_dir, vec3 view_dir, float shininess) {
  return pow(max(0.0, dot(reflected_dir, view_dir)), shininess * 30.0) *
         pow(shininess, 2.0);
}

void get_incoming_light(inout vec3 out_diffuse, inout vec3 out_specular,
                        int light_index, int kind, vec3 normal,
                        float shininess) {
  // TODO: Calculate light contribution in a physically based way. Don't have
  // the time to do this before LD53 though.

  // TODO: Handle spot and directional lights

  vec3 color = light_color_and_kind[light_index].rgb;
  vec3 to_light =
      (view_from_world * light_position[light_index]).xyz - view_pos;
  vec3 light_dir = normalize(to_light);
  float distance_squared = dot(to_light, to_light);

  float light_power = light_intensity_params[light_index].x / 1500.0;
  float k_diffuse = max(0.0, dot(normal, light_dir));
  float k_specular = 0.0;
  if (k_diffuse > 0.0) {
    vec3 reflected = reflect(light_dir, normal);
    k_specular = specular(reflected, VIEW_VECTOR, shininess);
  }

  out_diffuse += k_diffuse * color * light_power / distance_squared;
  out_specular += k_specular * color * light_power / distance_squared;
}

void main() {
  vec4 texel_base_color = texture(base_color_tex, tex_coords);
  vec2 texel_metallic_roughness =
      texture(metallic_roughness_tex, tex_coords).rg;
  vec3 texel_normal = texture(normal_tex, tex_coords).rgb * 2.0 - 1.0;
  float texel_occlusion = texture(occlusion_tex, tex_coords).r;
  vec4 texel_emissive = texture(emissive_tex, tex_coords);

  float pixel_alpha = texel_base_color.a * base_color_factor.a;
  if (pixel_alpha < 0.01)
    discard;
  vec3 pixel_base_color =
      texel_base_color.rgb * vertex_color * base_color_factor.rgb;
  float pixel_metallic = texel_metallic_roughness.x * material_params.x;
  float pixel_roughness = texel_metallic_roughness.y * material_params.y;

  vec3 tangent_space_normal =
      normalize(vec3(texel_normal.xy * material_params.z, texel_normal.z));
  vec3 vertex_bitangent =
      normalize(cross(vertex_normal, vertex_tangent.xyz) * vertex_tangent.w);
  vec3 pixel_normal =
      normalize(mat3(vertex_tangent.xyz, vertex_bitangent, vertex_normal) *
                tangent_space_normal);

  float pixel_occlusion = 1.0 + material_params.w * (texel_occlusion - 1.0);
  vec3 light_emitted = texel_emissive.rgb * emissive_factor.rgb;

  float ambient_brightness = 0.2 * pixel_occlusion;
  // Just to give a little shape to everything
  float ambient_fudge = -max(0.0, dot(-VIEW_VECTOR, pixel_normal)) * 0.1;
  vec3 light_diffuse = vec3(ambient_brightness + ambient_fudge);
  vec3 light_specular = vec3(0.0);
  for (int i = 0; i < MAX_LIGHTS; i++) {
    int kind = int(light_color_and_kind[i].w);
    if (kind == 0) {
      break;
    }
    get_incoming_light(light_diffuse, light_specular, i, kind, pixel_normal,
                       (1.0 - pixel_roughness));
  }

  vec3 light_outgoing_to_camera =
      light_emitted + pixel_base_color * light_diffuse + light_specular;
  vec3 output_linear_color = aces_filmic(light_outgoing_to_camera);

  // The framebuffer is not SRGB, so we transform the linear color to
  // close-enough-to-srgb.
  FRAG_COLOR = vec4(pow(output_linear_color, vec3(1.0 / 2.2)), pixel_alpha);
}
