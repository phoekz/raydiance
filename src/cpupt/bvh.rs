use super::*;

// Implementation based on PBRT's bounding volume hierarchy.

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Default, Debug)]
pub struct Node {
    pub bounds_mn: Point3,
    pub bounds_mx: Point3,
    pub offset: u32,
    pub primitive_count: u8,
    pub axis: u8,
    pub pad: u16,
}

pub fn create(triangles: &[Triangle]) -> (Vec<Node>, Vec<Triangle>) {
    // Primitives.
    let mut primitives = triangles
        .iter()
        .map(|triangle| {
            let mut bounds = Aabb::new();
            bounds.extend(&triangle.positions[0]);
            bounds.extend(&triangle.positions[1]);
            bounds.extend(&triangle.positions[2]);
            bounds
        })
        .enumerate()
        .map(|(primitive_id, bounds)| Primitive::from(primitive_id, bounds))
        .collect::<Vec<_>>();

    // Build.
    let mut build_nodes = vec![];
    let mut ordered_primitives = vec![];
    build_recursive(
        &mut primitives[0..triangles.len()],
        &mut build_nodes,
        &mut ordered_primitives,
    );

    // Flatten.
    let mut nodes = vec![];
    flatten(&build_nodes, 0, &mut nodes);

    // Sort triangle list in BVH order.
    let triangles = ordered_primitives
        .into_iter()
        .map(|primitive| triangles[primitive])
        .collect::<Vec<_>>();

    (nodes, triangles)
}

fn build_recursive(
    primitives: &mut [Primitive],
    build_nodes: &mut Vec<BuildNode>,
    ordered_primitives: &mut Vec<usize>,
) -> usize {
    const BUCKET_COUNT: usize = 12;
    const NODE_MAX_PRIMITIVE_COUNT: usize = 255;

    #[derive(Clone, Copy)]
    struct Bucket {
        count: usize,
        bounds: Aabb,
    }

    impl Default for Bucket {
        fn default() -> Self {
            Self {
                count: 0,
                bounds: Aabb::new(),
            }
        }
    }

    // Validation.
    assert!(!primitives.is_empty());

    // Make a new node.
    build_nodes.push(BuildNode::default());
    let curr = build_nodes.len() - 1;

    // Current bounds.
    let bounds = primitives.iter().fold(Aabb::new(), |bounds, primitive| {
        bounds.merged(&primitive.bounds)
    });

    // Only one primitive left, terminate as leaf.
    let primitive_count = primitives.len();
    if primitive_count == 1 {
        build_nodes[curr].set_leaf(ordered_primitives.len(), primitive_count, bounds);
        ordered_primitives.extend(primitives.iter().map(|primitive| primitive.id));
        return curr;
    }

    // Build inner node.
    let centroid_bounds = Aabb::from_points(primitives.iter().map(|primitive| &primitive.centroid));
    let (split_axis, _) = centroid_bounds.extents().argmax();

    // Degenerate bounds, terminate as leaf.
    if approx::ulps_eq!(
        centroid_bounds.max()[split_axis],
        centroid_bounds.min()[split_axis],
        max_ulps = 0
    ) {
        build_nodes[curr].set_leaf(ordered_primitives.len(), primitive_count, bounds);
        ordered_primitives.extend(primitives.iter().map(|primitive| primitive.id));
        return curr;
    }

    // Initial split point.
    let mut split = primitive_count / 2;

    // Reorder primitives according to SAH.
    if primitive_count <= 4 {
        // SAH computation is excessive, sort by centroid instead.
        primitives.sort_by(|primitive_a, primitive_b| {
            primitive_a.centroid[split_axis]
                .partial_cmp(&primitive_b.centroid[split_axis])
                .expect("Unable to compare floats")
        });
    } else {
        // Initialize buckets.
        let mut buckets = [Bucket::default(); BUCKET_COUNT];
        let find_bucket = |primitive: &Primitive| -> usize {
            let numer = primitive.centroid[split_axis] - centroid_bounds.min()[split_axis];
            let denom = centroid_bounds.max()[split_axis] - centroid_bounds.min()[split_axis];
            let bucket = (BUCKET_COUNT as f32 * numer / denom) as usize;
            bucket.min(BUCKET_COUNT - 1)
        };
        for primitive in primitives.iter() {
            let bucket = &mut buckets[find_bucket(primitive)];
            bucket.count += 1;
            bucket.bounds.merge(&primitive.bounds);
        }

        // Bruteforce SAH cost at every possible split point.
        let bounds_area = surface_area(&bounds);
        let mut costs = [0.0; BUCKET_COUNT - 1];
        costs.iter_mut().enumerate().for_each(|(i, cost)| {
            // Left split.
            let left = buckets[0..=i]
                .iter()
                .fold((0.0, Aabb::new()), |(count, bounds), bucket| {
                    (count + 1.0, bounds.merged(&bucket.bounds))
                });

            // Right split.
            let right = buckets[(i + 1)..BUCKET_COUNT]
                .iter()
                .fold((0.0, Aabb::new()), |(count, bounds), bucket| {
                    (count + 1.0, bounds.merged(&bucket.bounds))
                });

            *cost = 0.125 * (left.0 * surface_area(&left.1) + right.0 * surface_area(&right.1))
                / bounds_area;
        });

        // Find the bucket with the minimum SAH cost.
        let (min_cost_index, &min_cost) = costs
            .iter()
            .enumerate()
            .min_by(|&(_, &x), &(_, &y)| x.partial_cmp(&y).expect("Unable to compare floats"))
            .expect("Unable to find minimum SAH bucket");

        // Partition or terminate as leaf?
        let leaf_cost = primitive_count as f32;
        if primitive_count > NODE_MAX_PRIMITIVE_COUNT || min_cost < leaf_cost {
            // Partition around the best bucket.
            split = itertools::partition(primitives.iter_mut(), |primitive| {
                find_bucket(primitive) <= min_cost_index
            });
        } else {
            // Splitting is too expensive, terminate as leaf.
            build_nodes[curr].set_leaf(ordered_primitives.len(), primitive_count, bounds);
            ordered_primitives.extend(primitives.iter().map(|primitive| primitive.id));
            return curr;
        }
    }

    // Recurse.
    let left = build_recursive(&mut primitives[0..split], build_nodes, ordered_primitives);
    let right = build_recursive(&mut primitives[split..], build_nodes, ordered_primitives);
    let children_bounds = build_nodes[left].bounds.merged(&build_nodes[right].bounds);
    build_nodes[curr].set_interior(split_axis, [left, right], children_bounds);

    curr
}

fn flatten(build_nodes: &[BuildNode], parent: usize, nodes: &mut Vec<Node>) -> usize {
    // Make a new node.
    let curr = nodes.len();
    nodes.push(Node::default());

    // Copy bounds.
    nodes[curr].bounds_mn = build_nodes[parent].bounds.min();
    nodes[curr].bounds_mx = build_nodes[parent].bounds.max();

    // Leaf or interior.
    if build_nodes[parent].primitive_count.is_some() {
        // Leaf node.
        let parent = &build_nodes[parent];
        nodes[curr].offset = parent.first_primitive_offset.expect("Invalid BuildNode") as u32;
        nodes[curr].primitive_count = parent.primitive_count.expect("Invalid BuildNode") as u8;
    } else {
        // Interior node.
        let parent = &build_nodes[parent];
        nodes[curr].axis = parent.split_axis.expect("Invalid BuildNode") as u8;
        let children = parent.children.expect("Invalid BuildNode");

        // Recurse.
        flatten(build_nodes, children[0], nodes);
        nodes[curr].offset = flatten(build_nodes, children[1], nodes) as u32;
    }

    curr
}

#[derive(Debug)]
struct Primitive {
    id: usize,
    centroid: Point3,
    bounds: Aabb,
}

impl Primitive {
    fn from(id: usize, bounds: Aabb) -> Self {
        Self {
            id,
            centroid: bounds.center(),
            bounds,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BuildNode {
    bounds: Aabb,
    children: Option<[usize; 2]>,
    split_axis: Option<usize>,
    first_primitive_offset: Option<usize>,
    primitive_count: Option<usize>,
}

impl Default for BuildNode {
    fn default() -> Self {
        Self {
            bounds: Aabb::new(),
            children: None,
            split_axis: None,
            first_primitive_offset: None,
            primitive_count: None,
        }
    }
}

impl BuildNode {
    fn set_leaf(&mut self, first_primitive_offset: usize, primitive_count: usize, bounds: Aabb) {
        assert_eq!(self.bounds, Aabb::new());
        assert!(self.children.is_none());
        assert!(self.split_axis.is_none());
        assert!(self.first_primitive_offset.is_none());
        assert!(self.primitive_count.is_none());
        self.bounds = bounds;
        self.first_primitive_offset = Some(first_primitive_offset);
        self.primitive_count = Some(primitive_count);
    }

    fn set_interior(&mut self, split_axis: usize, children: [usize; 2], bounds: Aabb) {
        assert_eq!(self.bounds, Aabb::new());
        assert!(self.children.is_none());
        assert!(self.split_axis.is_none());
        assert!(self.first_primitive_offset.is_none());
        assert!(self.primitive_count.is_none());
        self.bounds = bounds;
        self.children = Some(children);
        self.split_axis = Some(split_axis);
    }
}

fn surface_area(bounds: &Aabb) -> f32 {
    let extents = bounds.extents();
    2.0 * (extents.x * extents.y + extents.x * extents.z + extents.y * extents.z)
}
