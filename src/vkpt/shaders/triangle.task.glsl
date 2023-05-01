#version 460 core
#include "common.glsl"

// Inputs.
layout(local_size_x = 1) in;
layout(local_size_y = 1) in;
layout(local_size_z = 1) in;

// Outputs.
taskPayloadSharedEXT TaskPayload task_payload;

uint mesh_group_count(uint triangle_count) {
    return (triangle_count + WORKGROUP_SIZE - 1) / WORKGROUP_SIZE;
}

void main() {
    // Task payload.
    const uint mesh_index = gl_GlobalInvocationID.x;
    task_payload.mesh_index = mesh_index;

    // Mesh shaders.
    const Mesh mesh = meshes[mesh_index];
    const uint mesh_group_x = mesh_group_count(mesh.triangle_count);
    EmitMeshTasksEXT(mesh_group_x, 1, 1);
}