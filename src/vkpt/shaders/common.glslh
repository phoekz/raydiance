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

layout(scalar, push_constant) uniform PushConstants {
    mat4 transform;
    vec4 base_color;
    uint mesh_index;
}
pc;

//
// Scene
//

// Mesh.
struct Mesh {
    mat4 transform;
    uint material;
    uint triangle_count;
};
layout(scalar, binding = 0) readonly buffer Meshes {
    Mesh meshes[];
};

// Positions.
layout(scalar, binding = 1) readonly buffer MeshPositions {
    vec3 ptr[];
}
mesh_positions[];

vec4 mesh_position(uint i) {
    vec3 position = mesh_positions[nonuniformEXT(pc.mesh_index)].ptr[i];
    return pc.transform * vec4(position, 1.0);
}

// Texture coordinates.
layout(scalar, binding = 2) readonly buffer MeshTexcoords {
    vec2 ptr[];
}
mesh_texcoords[];

vec2 mesh_texcoord(uint i) {
    return mesh_texcoords[nonuniformEXT(pc.mesh_index)].ptr[i];
}

// Normals.
layout(scalar, binding = 3) readonly buffer MeshNormals {
    vec3 ptr[];
}
mesh_normals[];

vec3 mesh_normal(uint i) {
    return mesh_normals[nonuniformEXT(pc.mesh_index)].ptr[i];
}

// Triangles.
layout(scalar, binding = 4) readonly buffer MeshTriangles {
    uvec3 ptr[];
}
mesh_triangles[];

uvec3 mesh_triangle(uint i) {
    return mesh_triangles[nonuniformEXT(pc.mesh_index)].ptr[i];
}

// Materials.
struct Material {
    uint base_color;
    uint metallic;
    uint roughness;
    uint specular;
    uint specular_tint;
    uint sheen;
    uint sheen_tint;
};
layout(scalar, binding = 5) readonly buffer Materials {
    Material materials[];
};

// Textures.
layout(binding = 6) uniform texture2D textures[];

// Samplers.
layout(binding = 7) uniform sampler samplers[];

//
// Mesh-fragment interface
//

struct FragmentVertex {
    vec2 texcoord;
    vec3 normal;
};

struct FragmentPrimitive {
    uint base_color;
};