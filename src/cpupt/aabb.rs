use super::*;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    extents: [na::Point3<f32>; 2],
}

impl Aabb {
    #[inline]
    pub fn new() -> Self {
        Self {
            extents: [
                na::Vector3::repeat(f32::MAX).into(),
                na::Vector3::repeat(-f32::MAX).into(),
            ],
        }
    }

    #[inline]
    pub fn from_min_max(min: &na::Point3<f32>, max: &na::Point3<f32>) -> Self {
        Self {
            extents: [*min, *max],
        }
    }

    pub fn from_points<'a, Iter>(points: Iter) -> Self
    where
        Iter: IntoIterator<Item = &'a na::Point3<f32>>,
    {
        let mut aabb = Self::new();
        for point in points {
            aabb.extend(point);
        }
        aabb
    }

    #[inline]
    pub fn min(&self) -> na::Point3<f32> {
        self.extents[0]
    }

    #[inline]
    pub fn max(&self) -> na::Point3<f32> {
        self.extents[1]
    }

    #[inline]
    pub fn center(&self) -> na::Point3<f32> {
        na::center(&self.min(), &self.max())
    }

    #[inline]
    pub fn extents(&self) -> na::Vector3<f32> {
        self.max() - self.min()
    }

    pub fn extend(&mut self, point: &na::Point3<f32>) {
        self.extents[0] = self.min().coords.inf(&point.coords).into();
        self.extents[1] = self.max().coords.sup(&point.coords).into();
    }

    pub fn merge(&mut self, other: &Aabb) {
        self.extents[0] = self.min().inf(&other.min());
        self.extents[1] = self.max().sup(&other.max());
    }

    pub fn merged(&self, other: &Aabb) -> Self {
        Self {
            extents: [self.min().inf(&other.min()), self.max().sup(&other.max())],
        }
    }
}

impl Default for Aabb {
    fn default() -> Self {
        Self::new()
    }
}
