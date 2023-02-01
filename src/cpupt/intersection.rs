use super::*;

pub struct RayTriangleIntersector {
    k: Vec3u,
    s: Vec3,
}

impl RayTriangleIntersector {
    // Implementation based on "Watertight Ray/Triangle Intersection".
    // https://jcgt.org/published/0002/01/05/

    pub fn new(ray: &Ray) -> Self {
        // Calculate dimension where the ray direction is maximal.
        let mut k = vector![0xffff_ffff, 0xffff_ffff, 0xffff_ffff];
        k.z = ray.dir.abs().argmax().0 as u32;
        k.x = k.z + 1;
        if k.x == 3 {
            k.x = 0;
        }
        k.y = k.x + 1;
        if k.y == 3 {
            k.y = 0;
        }

        // Swap kx and ky dimension to preserve winding direction of triangles.
        if ray.dir[k.z as usize] < 0.0 {
            let tmp = k.x;
            k.x = k.y;
            k.y = tmp;
        }

        // Calculate shear constants.
        let s = vector![
            ray.dir[k.x as usize] / ray.dir[k.z as usize],
            ray.dir[k.y as usize] / ray.dir[k.z as usize],
            1.0 / ray.dir[k.z as usize]
        ];

        Self { k, s }
    }

    pub fn hit(&self, ray: &Ray, triangle: &Triangle, out_t: &mut f32, out_uvw: &mut Vec3) -> bool {
        // Aliases.
        let k = self.k;
        let s = self.s;

        // Unpack triangle.
        let a = triangle.positions[0] - ray.origin;
        let b = triangle.positions[1] - ray.origin;
        let c = triangle.positions[2] - ray.origin;

        // Perform shear and scale of vertices.
        let ax = a[k.x as usize] - s.x * a[k.z as usize];
        let ay = a[k.y as usize] - s.y * a[k.z as usize];
        let bx = b[k.x as usize] - s.x * b[k.z as usize];
        let by = b[k.y as usize] - s.y * b[k.z as usize];
        let cx = c[k.x as usize] - s.x * c[k.z as usize];
        let cy = c[k.y as usize] - s.y * c[k.z as usize];

        // Calculate scaled barycentric coordinates.
        let mut u = cx * by - cy * bx;
        let mut v = ax * cy - ay * cx;
        let mut w = bx * ay - by * ax;

        // Fallback to test against edges using double precision.
        if u == 0.0 && v == 0.0 && w == 0.0 {
            let cxby = f64::from(cx) * f64::from(by);
            let cybx = f64::from(cy) * f64::from(bx);
            u = (cxby - cybx) as f32;
            let axcy = f64::from(ax) * f64::from(cy);
            let aycx = f64::from(ay) * f64::from(cx);
            v = (axcy - aycx) as f32;
            let bxay = f64::from(bx) * f64::from(ay);
            let byax = f64::from(by) * f64::from(ax);
            w = (bxay - byax) as f32;
        }

        // Perform edge tests.
        if u < 0.0 || v < 0.0 || w < 0.0 {
            return false;
        }

        // Calculate determinant.
        let det = u + v + w;
        if det == 0.0 {
            return false;
        }

        // Calculate scaled z-coordinates of vertices and use them to calculate the hit distance.
        let az = s.z * a[k.z as usize];
        let bz = s.z * b[k.z as usize];
        let cz = s.z * c[k.z as usize];
        let t = u * az + v * bz + w * cz;
        if t < 0.0 || t > *out_t * det {
            return false;
        }

        // Normalize.
        let rcpdet = 1.0 / det;
        *out_uvw = vector![u * rcpdet, v * rcpdet, w * rcpdet];

        // Update t-value.
        *out_t = t * rcpdet;

        true
    }
}

pub struct RayAabbIntersector {
    ray_dir_inv: Vec3,
    ray_dir_neg: Vec3b,
}

impl RayAabbIntersector {
    // Implementation based on PBRT.

    pub fn new(ray: &Ray) -> Self {
        let ray_dir_inv = vector![1.0 / ray.dir[0], 1.0 / ray.dir[1], 1.0 / ray.dir[2]];
        let ray_dir_neg = vector![
            ray_dir_inv.x < 0.0,
            ray_dir_inv.y < 0.0,
            ray_dir_inv.z < 0.0
        ];
        Self {
            ray_dir_inv,
            ray_dir_neg,
        }
    }

    #[inline]
    fn gamma(n: f32) -> f32 {
        const MACHINE_EPSILON: f32 = f32::EPSILON * 0.5;
        (n * MACHINE_EPSILON) / (1.0 - n * MACHINE_EPSILON)
    }

    pub fn hit(&self, ray: &Ray, aabb: &Aabb) -> bool {
        // Compute slab intervals.
        let mut mn_tx: f32;
        let mut mn_ty: f32;
        let mut mn_tz: f32;
        let mut mx_tx: f32;
        let mut mx_ty: f32;
        let mut mx_tz: f32;
        if self.ray_dir_neg[0] {
            mn_tx = aabb.max().x;
            mx_tx = aabb.min().x;
        } else {
            mn_tx = aabb.min().x;
            mx_tx = aabb.max().x;
        }
        if self.ray_dir_neg[1] {
            mn_ty = aabb.max().y;
            mx_ty = aabb.min().y;
        } else {
            mn_ty = aabb.min().y;
            mx_ty = aabb.max().y;
        }
        if self.ray_dir_neg[2] {
            mn_tz = aabb.max().z;
            mx_tz = aabb.min().z;
        } else {
            mn_tz = aabb.min().z;
            mx_tz = aabb.max().z;
        }
        mn_tx = (mn_tx - ray.origin.x) * self.ray_dir_inv.x;
        mn_ty = (mn_ty - ray.origin.y) * self.ray_dir_inv.y;
        mn_tz = (mn_tz - ray.origin.z) * self.ray_dir_inv.z;
        mx_tx = (mx_tx - ray.origin.x) * self.ray_dir_inv.x;
        mx_ty = (mx_ty - ray.origin.y) * self.ray_dir_inv.y;
        mx_tz = (mx_tz - ray.origin.z) * self.ray_dir_inv.z;

        // Ensures robust bounds intersection.
        mx_tx *= 1.0 + 2.0 * Self::gamma(3.0);
        mx_ty *= 1.0 + 2.0 * Self::gamma(3.0);
        mx_tz *= 1.0 + 2.0 * Self::gamma(3.0);

        // Check for intersections.
        if mn_tx > mx_ty || mn_ty > mx_tx {
            return false;
        }
        if mn_ty > mn_tx {
            mn_tx = mn_ty;
        }
        if mx_ty < mx_tx {
            mx_tx = mx_ty;
        }
        if mn_tx > mx_tz || mn_tz > mx_tx {
            return false;
        }
        if mn_tz > mn_tx {
            mn_tx = mn_tz;
        }
        if mx_tz < mx_tx {
            mx_tx = mx_tz;
        }

        (mn_tx < f32::INFINITY) && (mx_tx > 0.0)
    }
}

#[derive(Clone, Copy, Default, Debug)]
pub struct RayBvhHitStats {
    pub rays: u64,
    pub ray_triangle_tests: u64,
    pub ray_triangle_hits: u64,
    pub ray_aabb_tests: u64,
    pub ray_aabb_hits: u64,
}

impl std::ops::AddAssign for RayBvhHitStats {
    fn add_assign(&mut self, rhs: Self) {
        self.ray_triangle_tests = rhs.ray_triangle_tests;
        self.ray_triangle_hits = rhs.ray_triangle_hits;
        self.ray_aabb_tests = rhs.ray_aabb_tests;
        self.ray_aabb_hits = rhs.ray_aabb_hits;
    }
}

pub fn ray_bvh_hit(
    ray: &Ray,
    nodes: &[bvh::Node],
    triangles: &[Triangle],
    out_closest_hit: &mut f32,
    out_barycentrics: &mut Vec3,
    out_triangle_index: &mut u32,
    stats: &mut RayBvhHitStats,
) -> bool {
    let ray_triangle = RayTriangleIntersector::new(ray);
    let ray_aabb = RayAabbIntersector::new(ray);

    stats.rays += 1;

    let mut node_index = 0;
    let mut todo_offset = 0;
    let mut todo = [0; 64];

    let mut best_closest_hit = f32::MAX;
    let mut hit = false;

    loop {
        // Unpack bvh node.
        let bvh_node = &nodes[node_index];
        let bounds = Aabb::from_min_max(&bvh_node.bounds_mn, &bvh_node.bounds_mx);

        stats.ray_aabb_tests += 1;
        if ray_aabb.hit(ray, &bounds) {
            stats.ray_aabb_hits += 1;
            let offset = bvh_node.offset;
            let primitive_count = bvh_node.primitive_count;
            let axis = bvh_node.axis;
            if primitive_count > 0 {
                // Intersect leaf node triangles.
                for primitive_index in 0..primitive_count {
                    // Unpack triangle.
                    let triangle_index = offset + u32::from(primitive_index);
                    let triangle = triangles[triangle_index as usize];

                    // Intersect triangle.
                    let mut closest_hit = f32::MAX;
                    let mut barycentrics = vector![0.0, 0.0, 0.0];
                    stats.ray_triangle_tests += 1;
                    if ray_triangle.hit(ray, &triangle, &mut closest_hit, &mut barycentrics) {
                        stats.ray_triangle_hits += 1;
                        hit = true;

                        if closest_hit < best_closest_hit {
                            *out_closest_hit = closest_hit;
                            *out_barycentrics = barycentrics;
                            *out_triangle_index = triangle_index;
                            best_closest_hit = closest_hit;
                        }
                    }
                }

                // End traversal.
                if todo_offset == 0 {
                    break;
                }

                // Pop.
                todo_offset -= 1;
                node_index = todo[todo_offset];
            } else if ray_aabb.ray_dir_neg[axis as usize] {
                todo[todo_offset] = node_index + 1;
                todo_offset += 1;
                node_index = offset as usize;
            } else {
                todo[todo_offset] = offset as usize;
                todo_offset += 1;
                node_index += 1;
            }
        } else {
            // End traversal.
            if todo_offset == 0 {
                break;
            }

            // Pop.
            todo_offset -= 1;
            node_index = todo[todo_offset];
        }
    }

    hit
}
