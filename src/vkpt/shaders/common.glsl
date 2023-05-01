//
// Extensions
//

#extension GL_EXT_mesh_shader : require
#extension GL_EXT_scalar_block_layout : require
#extension GL_EXT_nonuniform_qualifier : require

//
// Constants
//

#define WORKGROUP_SIZE 32
#define MAX_VERTICES (3 * WORKGROUP_SIZE)
#define MAX_PRIMITIVES WORKGROUP_SIZE

//
// Push constants
//

layout(push_constant, scalar) uniform PushConstants {
    mat4 transform;
}
pc;

//
// Task-mesh payload
//

struct TaskPayload {
    uint mesh_index;
};

//
// Scene
//

struct Mesh {
    mat4 transform;
    uint material;
    uint triangle_count;
};

struct Material {
    uint base_color;
    uint metallic;
    uint roughness;
    uint specular;
    uint specular_tint;
    uint sheen;
    uint sheen_tint;
};

// clang-format off
layout(binding = 0, scalar) readonly buffer Meshes { Mesh meshes[]; };
layout(binding = 1, scalar) readonly buffer MeshPositions { vec3 ptr[]; } mesh_positions[];
layout(binding = 2, scalar) readonly buffer MeshTexcoords { vec2 ptr[]; } mesh_texcoords[];
layout(binding = 3, scalar) readonly buffer MeshNormals { vec3 ptr[]; } mesh_normals[];
layout(binding = 4, scalar) readonly buffer MeshTriangles { uvec3 ptr[]; } mesh_triangles[];
layout(binding = 5, scalar) readonly buffer Materials { Material materials[]; };
layout(binding = 6) uniform texture2D textures[];
layout(binding = 7, scalar) readonly buffer TextureFlags { uint texture_flags[]; };
layout(binding = 8, scalar) readonly buffer DynamicTextures { vec4 dynamic_textures[]; };
layout(binding = 9) uniform sampler samplers[];
// clang-format on

//
// Fetching
//

vec3 mesh_position(uint mi, uint i) {
    return mesh_positions[nonuniformEXT(mi)].ptr[i];
}

vec2 mesh_texcoord(uint mi, uint i) {
    return mesh_texcoords[nonuniformEXT(mi)].ptr[i];
}

vec3 mesh_normal(uint mi, uint i) {
    return mesh_normals[nonuniformEXT(mi)].ptr[i];
}

uvec3 mesh_triangle(uint mi, uint i) {
    return mesh_triangles[nonuniformEXT(mi)].ptr[i];
}

vec4 sample_texture(uint ti, uint si, vec2 texcoord) {
    return texture(sampler2D(textures[nonuniformEXT(ti)], samplers[nonuniformEXT(si)]), texcoord);
}

//
// Transforms
//

vec4 clip_position(Mesh mesh, uint mi, uint i) {
    return pc.transform * mesh.transform * vec4(mesh_position(mi, i), 1.0);
}