#version 460 core
#include "common.glsl"

layout(local_size_x = WORKGROUP_SIZE) in;
layout(local_size_y = 1) in;
layout(local_size_z = 1) in;

layout(triangles) out;
layout(max_vertices = MAX_VERTICES) out;
layout(max_primitives = MAX_PRIMITIVES) out;

layout(location = 0) out FragmentVertex fragment_vertices[];
layout(location = 2)
    perprimitiveEXT flat out FragmentPrimitive fragment_primitives[];

void main() {
    // Input.
    Mesh mesh = meshes[pc.mesh_index];
    Material material = materials[mesh.material];

    // Source primitive.
    uint src_primitive_index = gl_GlobalInvocationID.x;
    uvec3 src = mesh_triangle(src_primitive_index);

    // Avoid overruns.
    if (src_primitive_index >= mesh.triangle_count) {
        return;
    }

    // Destination primitive.
    uint dst_primitive_index = gl_LocalInvocationID.x;
    uvec3 dst = 3 * dst_primitive_index + uvec3(0, 1, 2);

    // Only the first invocation in a workgroup sets mesh output sizes.
    if (dst_primitive_index == 0) {
        uint remaining_count =
            mesh.triangle_count - gl_WorkGroupID.x * gl_WorkGroupSize.x;
        uint primitive_count = min(gl_WorkGroupSize.x, remaining_count);
        uint vertex_count = 3 * primitive_count;
        SetMeshOutputsEXT(vertex_count, primitive_count);
    }

    // Output.
    gl_MeshVerticesEXT[dst[0]].gl_Position = mesh_position(src[0]);
    gl_MeshVerticesEXT[dst[1]].gl_Position = mesh_position(src[1]);
    gl_MeshVerticesEXT[dst[2]].gl_Position = mesh_position(src[2]);
    fragment_vertices[dst[0]].texcoord = mesh_texcoord(src[0]);
    fragment_vertices[dst[1]].texcoord = mesh_texcoord(src[1]);
    fragment_vertices[dst[2]].texcoord = mesh_texcoord(src[2]);
    fragment_vertices[dst[0]].normal = mesh_normal(src[0]);
    fragment_vertices[dst[1]].normal = mesh_normal(src[1]);
    fragment_vertices[dst[2]].normal = mesh_normal(src[2]);
    gl_PrimitiveTriangleIndicesEXT[dst_primitive_index][0] = dst[0];
    gl_PrimitiveTriangleIndicesEXT[dst_primitive_index][1] = dst[1];
    gl_PrimitiveTriangleIndicesEXT[dst_primitive_index][2] = dst[2];
    fragment_primitives[dst_primitive_index].base_color = material.base_color;
}
