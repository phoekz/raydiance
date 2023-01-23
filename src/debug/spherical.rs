use super::*;

#[derive(Clone, Copy)]
pub struct Spherical {
    angle_xz: f32, // pbrt:phi, azimuthal, [-pi,pi]
    angle_y: f32,  // pbrt:theta, polar, [0,pi]
}

#[derive(Clone, Copy)]
pub struct NormalizedSpherical {
    angle_xz: f32, // pbrt:phi, azimuthal, [0,1]
    angle_y: f32,  // pbrt:theta, polar, [0,1]
}

impl Spherical {
    pub fn new(angle_xz: f32, angle_y: f32) -> Self {
        assert!((-PI..=PI).contains(&angle_xz));
        assert!((0.0..=PI).contains(&angle_y));
        Self { angle_xz, angle_y }
    }

    pub fn angle_xz(self) -> f32 {
        self.angle_xz
    }

    pub fn angle_y(self) -> f32 {
        self.angle_y
    }

    pub fn to_cartesian(self) -> na::UnitVector3<f32> {
        na::Unit::new_normalize(na::vector![
            self.angle_y.sin() * self.angle_xz.cos(),
            self.angle_y.cos(),
            self.angle_y.sin() * self.angle_xz.sin()
        ])
    }

    pub fn from_cartesian(c: na::UnitVector3<f32>) -> Self {
        let angle_xz = c.z.atan2(c.x);
        let angle_y = c.y.acos();
        assert!((-PI..=PI).contains(&angle_xz));
        assert!((0.0..=PI).contains(&angle_y));
        Self { angle_xz, angle_y }
    }

    pub fn normalized(self) -> NormalizedSpherical {
        NormalizedSpherical {
            angle_xz: (self.angle_xz + PI) / TAU,
            angle_y: self.angle_y / PI,
        }
    }

    pub fn from_normalized(norm: NormalizedSpherical) -> Self {
        Self {
            angle_xz: (norm.angle_xz - 0.5) * TAU,
            angle_y: norm.angle_y * 0.5 * PI,
        }
    }
}

impl NormalizedSpherical {
    pub fn new(angle_xz: f32, angle_y: f32) -> Self {
        assert!((0.0..=1.0).contains(&angle_xz));
        assert!((0.0..=1.0).contains(&angle_y));
        Self { angle_xz, angle_y }
    }

    pub fn angle_xz(self) -> f32 {
        self.angle_xz
    }

    pub fn angle_y(self) -> f32 {
        self.angle_y
    }
}

#[test]
fn test_round_trip() {
    let epsilon = 0.001;
    let samples = 50;
    for angle_xz in 0..samples {
        let angle_xz = (angle_xz as f32 + 0.5) / samples as f32;
        let angle_xz = (angle_xz - 0.5) * TAU;

        let angle_y = PI / 2.0;
        let src = Spherical::new(angle_xz, angle_y);
        let v = src.to_cartesian();
        let dst = Spherical::from_cartesian(v);
        println!(
            "angle_xz={:.06} angle_y={:.06} => \
            v=({:.06},{:.06},{:.06}) => \
            angle_xz={:.06} angle_y={:.06}",
            src.angle_xz(),
            src.angle_y(),
            v.x,
            v.y,
            v.z,
            dst.angle_xz(),
            dst.angle_y()
        );
        approx::assert_abs_diff_eq!(src.angle_xz(), dst.angle_xz(), epsilon = epsilon);
        approx::assert_abs_diff_eq!(src.angle_y(), dst.angle_y(), epsilon = epsilon);
    }
    println!();
    for angle_y in 0..samples {
        let angle_y = (angle_y as f32 + 0.5) / samples as f32;
        let angle_y = angle_y * PI;

        let angle_xz = 0.0;
        let src = Spherical::new(angle_xz, angle_y);
        let v = src.to_cartesian();
        let dst = Spherical::from_cartesian(v);
        println!(
            "angle_xz={:.06} angle_y={:.06} => \
            v=({:.06},{:.06},{:.06}) => \
            angle_xz={:.06} angle_y={:.06}",
            src.angle_xz(),
            src.angle_y(),
            v.x,
            v.y,
            v.z,
            dst.angle_xz(),
            dst.angle_y()
        );
        approx::assert_abs_diff_eq!(src.angle_xz(), dst.angle_xz(), epsilon = epsilon);
        approx::assert_abs_diff_eq!(src.angle_y(), dst.angle_y(), epsilon = epsilon);
    }
}
