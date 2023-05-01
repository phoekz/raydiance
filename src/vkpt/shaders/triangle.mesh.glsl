#version 460 core
#include "common.glsl"

// Inputs.
taskPayloadSharedEXT TaskPayload task_payload;
layout(local_size_x = WORKGROUP_SIZE) in;
layout(local_size_y = 1) in;
layout(local_size_z = 1) in;

// Outputs.
layout(triangles) out;
layout(max_vertices = MAX_VERTICES) out;
layout(max_primitives = MAX_PRIMITIVES) out;
layout(location = 0) out vec2 out_texcoord[];
layout(location = 1) out vec3 out_normal[];
layout(location = 2) perprimitiveEXT flat out uint out_base_color[];

void main() {
    // Input.
    const uint mesh_index = task_payload.mesh_index;
    const Mesh mesh = meshes[mesh_index];
    const Material material = materials[mesh.material];
    const uint src_primitive_index = gl_GlobalInvocationID.x;
    const uvec3 src = mesh_triangle(mesh_index, src_primitive_index);

    // Avoid overruns.
    if (src_primitive_index >= mesh.triangle_count) {
        return;
    }

    // Destination primitive.
    const uint dst_primitive_index = gl_LocalInvocationID.x;
    const uvec3 dst = 3 * dst_primitive_index + uvec3(0, 1, 2);

    // Only the first invocation in a workgroup sets mesh output sizes.
    if (dst_primitive_index == 0) {
        const uint remaining_count = mesh.triangle_count - gl_WorkGroupID.x * gl_WorkGroupSize.x;
        const uint primitive_count = min(gl_WorkGroupSize.x, remaining_count);
        const uint vertex_count = 3 * primitive_count;
        SetMeshOutputsEXT(vertex_count, primitive_count);
    }

    // Output.
    gl_MeshVerticesEXT[dst[0]].gl_Position = clip_position(mesh, mesh_index, src[0]);
    gl_MeshVerticesEXT[dst[1]].gl_Position = clip_position(mesh, mesh_index, src[1]);
    gl_MeshVerticesEXT[dst[2]].gl_Position = clip_position(mesh, mesh_index, src[2]);
    out_texcoord[dst[0]] = mesh_texcoord(mesh_index, src[0]);
    out_texcoord[dst[1]] = mesh_texcoord(mesh_index, src[1]);
    out_texcoord[dst[2]] = mesh_texcoord(mesh_index, src[2]);
    out_normal[dst[0]] = mesh_normal(mesh_index, src[0]);
    out_normal[dst[1]] = mesh_normal(mesh_index, src[1]);
    out_normal[dst[2]] = mesh_normal(mesh_index, src[2]);
    gl_PrimitiveTriangleIndicesEXT[dst_primitive_index] = dst;
    out_base_color[dst_primitive_index] = material.base_color;
}
