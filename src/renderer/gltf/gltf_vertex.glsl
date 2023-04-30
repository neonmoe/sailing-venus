#version 300 es
layout(location = 0) in vec3 POSITION;
layout(location = 1) in vec3 NORMAL;
layout(location = 2) in vec4 TANGENT;
layout(location = 3) in vec2 TEXCOORD_0;
layout(location = 4) in vec2 TEXCOORD_1;
layout(location = 5) in vec3 COLOR_0;
layout(location = 6) in mat4 MODEL_TRANSFORM;
layout(location = 10) in mat4 TEXCOORD_TRANSFORM;

out vec3 view_pos;
out vec3 vertex_color;
out vec3 vertex_normal;
out vec4 vertex_tangent;
out vec2 tex_coords;

uniform mat4 proj_from_view;
uniform mat4 view_from_world;

void main() {
  // TODO: Move the inverse transpose of the model transfrom to the cpu
  // TODO: Move the inverse of view_from_world to the cpu
  mat4 view_from_model = view_from_world * MODEL_TRANSFORM;
  mat3 inverse_transpose_model_transfrom =
      transpose(inverse(mat3(view_from_model)));
  vec4 view_pos_full = view_from_model * vec4(POSITION, 1.0);
  view_pos = view_pos_full.xyz;
  vertex_color = COLOR_0;
  vertex_normal = normalize(inverse_transpose_model_transfrom * NORMAL);
  vertex_tangent =
      vec4(normalize(mat3(view_from_model) * TANGENT.xyz), TANGENT.w);
  tex_coords = (TEXCOORD_TRANSFORM * vec4(TEXCOORD_0, 0.0, 1.0)).xy;
  gl_Position = proj_from_view * view_pos_full;
}
