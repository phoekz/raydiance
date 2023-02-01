use super::*;

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    extents: [Point3; 2],
}

impl Aabb {
    #[inline]
    pub fn new() -> Self {
        Self {
            extents: [
                Vec3::repeat(f32::MAX).into(),
                Vec3::repeat(-f32::MAX).into(),
            ],
        }
    }

    #[inline]
    pub fn from_min_max(min: &Point3, max: &Point3) -> Self {
        Self {
            extents: [*min, *max],
        }
    }

    pub fn from_points<'a, Iter>(points: Iter) -> Self
    where
        Iter: IntoIterator<Item = &'a Point3>,
    {
        let mut aabb = Self::new();
        for point in points {
            aabb.extend(point);
        }
        aabb
    }

    #[inline]
    pub fn min(&self) -> Point3 {
        self.extents[0]
    }

    #[inline]
    pub fn max(&self) -> Point3 {
        self.extents[1]
    }

    #[inline]
    pub fn center(&self) -> Point3 {
        na::center(&self.min(), &self.max())
    }

    #[inline]
    pub fn extents(&self) -> Vec3 {
        self.max() - self.min()
    }

    pub fn extend(&mut self, point: &Point3) {
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
