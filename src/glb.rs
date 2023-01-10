use super::*;

pub struct Scene {
    pub cameras: Vec<Camera>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

pub struct Camera {
    pub name: String,
    pub transform: na::Matrix4<f32>,
    pub aspect_ratio: f32,
    pub yfov: f32,
    pub znear: f32,
    pub zfar: f32,
}

pub struct Mesh {
    pub name: String,
    pub transform: na::Matrix4<f32>,
    pub positions: Positions,
    pub normals: Normals,
    pub triangles: Triangles,
    pub material: u32,
}

pub struct Positions(pub Vec<na::Point3<f32>>);
pub struct Normals(pub Vec<na::UnitVector3<f32>>);
pub struct Triangles(pub Vec<na::Vector3<u32>>);

pub struct Material {
    pub name: String,
    pub base_color: LinSrgba,
    pub metallic: f32,
    pub roughness: f32,
}

impl Scene {
    pub fn create(glb: &[u8]) -> Result<Self> {
        let mut scene = Self {
            cameras: vec![],
            meshes: vec![],
            materials: vec![],
        };
        let (gltf_document, gltf_buffer_data, _) =
            gltf::import_slice(glb).context("Importing gltf model")?;
        for gltf_scene in gltf_document.scenes() {
            for gltf_node in gltf_scene.nodes() {
                if let Some(gltf_camera) = gltf_node.camera() {
                    scene.cameras.push(Camera::new(&gltf_node, &gltf_camera)?);
                } else if let Some(gltf_mesh) = gltf_node.mesh() {
                    use gltf::mesh::Mode;
                    ensure!(gltf_mesh.primitives().count() == 1);
                    let gltf_primitive = gltf_mesh.primitives().next().unwrap();
                    if !matches!(gltf_primitive.mode(), Mode::Triangles) {
                        bail!("Only triangle meshes are supported right now");
                    }
                    scene.meshes.push(Mesh::new(
                        &gltf_node,
                        &gltf_mesh,
                        &gltf_primitive,
                        &gltf_buffer_data[0].0[..],
                    )?);
                    scene
                        .materials
                        .push(Material::new(&gltf_primitive.material())?);
                }
            }
        }
        Ok(scene)
    }
}

impl Camera {
    fn new(gltf_node: &gltf::Node, gltf_camera: &gltf::Camera) -> Result<Self> {
        use gltf::camera::Projection;

        let name = gltf_camera
            .name()
            .context("Camera must define a name")?
            .to_owned();

        let matrix = gltf_node.transform().matrix();
        let transform: na::Matrix4<f32> = unsafe { transmute(matrix) };

        if let Projection::Perspective(perspective) = gltf_camera.projection() {
            let aspect_ratio = perspective
                .aspect_ratio()
                .context("Cameras must define aspect ratio")?;
            let yfov = perspective.yfov();
            let znear = perspective.znear();
            let zfar = perspective.zfar().context("Cameras must define zfar")?;

            Ok(Self {
                name,
                transform,
                aspect_ratio,
                yfov,
                znear,
                zfar,
            })
        } else {
            bail!("Only perspective cameras are supported right now");
        }
    }

    pub fn clip_from_view(&self) -> na::Perspective3<f32> {
        na::Perspective3::new(self.aspect_ratio, self.yfov, self.znear, self.zfar)
    }

    pub fn world_from_view(&self) -> na::Matrix4<f32> {
        self.transform
    }

    pub fn position(&self) -> na::Point3<f32> {
        let coords: na::Vector3<f32> = self.transform.column(3).fixed_rows::<3>(0).into();
        na::Point3::from(coords)
    }
}

impl Mesh {
    fn new(
        gltf_node: &gltf::Node,
        gltf_mesh: &gltf::Mesh,
        gltf_primitive: &gltf::Primitive,
        gltf_buffer_data: &[u8],
    ) -> Result<Self> {
        use gltf::mesh::Semantic;

        let name = gltf_mesh
            .name()
            .context("Mesh must define a name")?
            .to_owned();

        let matrix = gltf_node.transform().matrix();
        let transform: na::Matrix4<f32> = unsafe { transmute(matrix) };

        let mut positions_accessor = None;
        let mut normals_accessor = None;
        let mut indices_accessor = None;
        for attribute in gltf_primitive.attributes() {
            let (semantic, accessor) = attribute;
            if matches!(semantic, Semantic::Positions) {
                positions_accessor = Some(accessor.clone());
            }
            if matches!(semantic, Semantic::Normals) {
                normals_accessor = Some(accessor.clone());
            }
        }
        if let Some(accessor) = gltf_primitive.indices() {
            indices_accessor = Some(accessor.clone());
        }
        let positions_accessor =
            positions_accessor.with_context(|| format!("Mesh '{name}' is missing positions"))?;
        let normals_accessor =
            normals_accessor.with_context(|| format!("Mesh '{name}' is missing normals"))?;
        let indices_accessor =
            indices_accessor.with_context(|| format!("Mesh '{name}' is missing indices"))?;
        let positions = Positions::new(&positions_accessor, gltf_buffer_data)?;
        let normals = Normals::new(&normals_accessor, gltf_buffer_data)?;
        let triangles = Triangles::new(&indices_accessor, gltf_buffer_data)?;

        let material = gltf_primitive
            .material()
            .index()
            .context("Mesh must define a material")? as u32;

        Ok(Self {
            name,
            transform,
            positions,
            normals,
            triangles,
            material,
        })
    }

    pub fn triangle_count(&self) -> u32 {
        self.triangles.count()
    }

    pub fn index_count(&self) -> u32 {
        self.triangles.index_count()
    }
}

impl Positions {
    fn new(acc: &gltf::Accessor, data: &[u8]) -> Result<Self> {
        use gltf::accessor::DataType;
        use gltf::accessor::Dimensions;

        ensure!(acc.data_type() == DataType::F32);
        ensure!(acc.dimensions() == Dimensions::Vec3);
        ensure!(acc.size() == 3 * size_of::<f32>());
        ensure!(acc.offset() == 0);
        ensure!(acc.normalized() == false);
        let view = acc.view().context("Accessor must have a buffer view")?;
        let offset = view.offset();
        let length = view.length();
        ensure!(view.stride().is_none());
        ensure!(length > 0);
        ensure!(length % 3 * size_of::<f32>() == 0);
        let slice_u8 = &data[offset..(offset + length)];
        let slice_vec3: &[na::Point3<f32>] = bytemuck::cast_slice(slice_u8);

        Ok(Self(slice_vec3.to_vec()))
    }
}

impl Deref for Positions {
    type Target = [na::Point3<f32>];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Normals {
    fn new(acc: &gltf::Accessor, data: &[u8]) -> Result<Self> {
        use gltf::accessor::DataType;
        use gltf::accessor::Dimensions;

        ensure!(acc.data_type() == DataType::F32);
        ensure!(acc.dimensions() == Dimensions::Vec3);
        ensure!(acc.size() == 3 * size_of::<f32>());
        ensure!(acc.offset() == 0);
        ensure!(acc.normalized() == false);
        let view = acc.view().context("Accessor must have a buffer view")?;
        let offset = view.offset();
        let length = view.length();
        ensure!(view.stride().is_none());
        ensure!(length > 0);
        ensure!(length % 3 * size_of::<f32>() == 0);
        let slice_u8 = &data[offset..(offset + length)];
        let slice_vec3: &[na::UnitVector3<f32>] = bytemuck::cast_slice(slice_u8);

        Ok(Self(slice_vec3.to_vec()))
    }
}

impl Deref for Normals {
    type Target = [na::UnitVector3<f32>];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Triangles {
    fn new(acc: &gltf::Accessor, data: &[u8]) -> Result<Self> {
        use gltf::accessor::DataType;
        use gltf::accessor::Dimensions;

        ensure!(acc.data_type() == DataType::U16 || acc.data_type() == DataType::U32);
        ensure!(acc.dimensions() == Dimensions::Scalar);
        ensure!(acc.size() == size_of::<u16>() || acc.size() == size_of::<u32>());
        ensure!(acc.offset() == 0);
        ensure!(acc.normalized() == false);
        let view = acc.view().context("Accessor must have a buffer view")?;
        let offset = view.offset();
        let length = view.length();
        ensure!(view.stride().is_none());
        ensure!(length > 0);
        let slice_u8 = &data[offset..(offset + length)];
        let slice_vec3 = match acc.data_type() {
            DataType::U16 => {
                ensure!(length % 3 * size_of::<u16>() == 0);
                let slice_u16: &[u16] = bytemuck::cast_slice(slice_u8);
                slice_u16
                    .chunks_exact(3)
                    .map(|chunk| {
                        na::vector![
                            u32::from(chunk[0]),
                            u32::from(chunk[1]),
                            u32::from(chunk[2])
                        ]
                    })
                    .collect::<Vec<_>>()
            }
            DataType::U32 => {
                ensure!(length % 3 * size_of::<u32>() == 0);

                bytemuck::cast_slice(slice_u8).to_vec()
            }
            _ => unreachable!(),
        };

        Ok(Self(slice_vec3))
    }

    fn count(&self) -> u32 {
        self.0.len() as u32
    }

    fn index_count(&self) -> u32 {
        3 * self.count()
    }
}

impl Deref for Triangles {
    type Target = [na::Vector3<u32>];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl Material {
    fn new(gltf_material: &gltf::Material) -> Result<Self> {
        use gltf::material::AlphaMode;

        let name = gltf_material
            .name()
            .context("Material must define a name")?
            .to_owned();

        ensure!(gltf_material.alpha_cutoff().is_none());
        ensure!(gltf_material.alpha_mode() == AlphaMode::Opaque);
        ensure!(gltf_material.double_sided() == false);
        ensure!(gltf_material.normal_texture().is_none());
        ensure!(gltf_material.occlusion_texture().is_none());
        ensure!(gltf_material.emissive_texture().is_none());
        let pbr = gltf_material.pbr_metallic_roughness();
        ensure!(pbr.base_color_texture().is_none());
        ensure!(pbr.metallic_roughness_texture().is_none());
        let base_color_factor = pbr.base_color_factor();
        let base_color = LinSrgba::new(
            base_color_factor[0],
            base_color_factor[1],
            base_color_factor[2],
            base_color_factor[3],
        );
        let metallic = pbr.metallic_factor();
        let roughness = pbr.roughness_factor();

        Ok(Self {
            name,
            base_color,
            metallic,
            roughness,
        })
    }
}
