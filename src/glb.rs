use super::*;

#[derive(Clone, Debug)]
pub struct Scene {
    pub cameras: Vec<Camera>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub bounding_box: Aabb,
    pub bounding_sphere: BoundingSphere,
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub name: String,
    pub transform: Mat4,
    pub aspect_ratio: f32,
    pub yfov: f32,
    pub znear: f32,
    pub zfar: f32,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub name: String,
    pub transform: Mat4,
    pub positions: Vec<Point3>,
    pub tex_coords: Vec<Point2>,
    pub normals: Vec<Normal>,
    pub triangles: Vec<Vec3u>,
    pub material: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialModel {
    Diffuse,
    Disney,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialField {
    BaseColor,
    Metallic,
    Roughness,
    Specular,
    SpecularTint,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub model: MaterialModel,
    pub base_color: u32,
    pub metallic: u32,
    pub roughness: u32,
    pub specular: u32,
    pub specular_tint: u32,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Texture {
    Scalar(f32),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    Image {
        width: u32,
        height: u32,
        components: u32,
        pixels: Vec<f32>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DynamicScene {
    pub materials: Vec<DynamicMaterial>,
    pub textures: Vec<DynamicTexture>,
    pub default_textures: Vec<DynamicTexture>,
    pub replaced_textures: BitVec,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicMaterial {
    pub model: MaterialModel,
    pub base_color: u32,
    pub metallic: u32,
    pub roughness: u32,
    pub specular: u32,
    pub specular_tint: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DynamicTexture {
    Scalar(f32),
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
}

//
// Scene
//

impl Scene {
    pub fn create(glb: &[u8]) -> Result<(Self, DynamicScene)> {
        // Assets.
        let mut cameras = vec![];
        let mut meshes = vec![];
        let mut materials = vec![];
        let mut textures = vec![];

        // Import.
        let (gltf_document, gltf_buffer_data, gltf_image_data) =
            gltf::import_slice(glb).context("Importing gltf model")?;

        // Traverse.
        for gltf_scene in gltf_document.scenes() {
            for gltf_node in gltf_scene.nodes() {
                // Cameras.
                if let Some(gltf_camera) = gltf_node.camera() {
                    import_gltf_camera(&gltf_camera, &gltf_node, &mut cameras)?;
                }
                // Meshes, materials, textures.
                else if let Some(gltf_mesh) = gltf_node.mesh() {
                    import_gltf_mesh(
                        &gltf_mesh,
                        &gltf_node,
                        &gltf_buffer_data,
                        &gltf_image_data,
                        &mut meshes,
                        &mut materials,
                        &mut textures,
                    )?;
                }
            }
        }

        // Validate.
        {
            let mut unique_mesh_names = HashSet::new();
            for mesh in &meshes {
                let name = mesh.name.as_str();
                let was_unique = unique_mesh_names.insert(name);
                ensure!(was_unique, "Mesh name {name} is not unique!",);
            }

            let mut unique_material_names = HashSet::new();
            for material in &materials {
                let name = material.name.as_str();
                let was_unique = unique_material_names.insert(name);
                ensure!(was_unique, "Material name {name} is not unique!",);
            }
        }

        // Stats.
        {
            info!("Scene contains {} cameras", cameras.len());
            info!("Scene contains {} meshes", meshes.len());
            for mesh in &meshes {
                info!(
                    "  {}: vertices={}, triangles={}",
                    &mesh.name,
                    mesh.positions.len(),
                    mesh.triangle_count()
                );
            }
            info!("Scene contains {} materials", materials.len());
            for material in &materials {
                let base_color = &textures[material.base_color as usize];
                let metallic = &textures[material.metallic as usize];
                let roughness = &textures[material.roughness as usize];
                let specular = &textures[material.specular as usize];
                let specular_tint = &textures[material.specular_tint as usize];
                let base_color = base_color.sample(Point2::new(0.5, 0.5));
                let metallic = metallic.sample(Point2::new(0.5, 0.5)).r();
                let roughness = roughness.sample(Point2::new(0.5, 0.5)).r();
                let specular = specular.sample(Point2::new(0.5, 0.5)).r();
                let specular_tint = specular_tint.sample(Point2::new(0.5, 0.5)).r();
                info!("  {}:", material.name);
                info!(
                    "    base_color=({}, {:.03})",
                    material.base_color, base_color
                );
                info!("    metallic=({}, {:.03})", material.metallic, metallic);
                info!("    roughness=({}, {:.03})", material.roughness, roughness);
                info!("    specular=({}, {:.03})", material.specular, specular);
                info!(
                    "    specular_tint=({}, {:.03})",
                    material.specular_tint, specular_tint
                );
            }
            info!("Scene contains {} textures", textures.len());
        }

        // Bounds.
        let (bounding_box, bounding_sphere) = {
            let mut bounding_box = Aabb::new();
            for mesh in &meshes {
                let transform = mesh.transform;
                for position in &mesh.positions {
                    let world_position = transform.transform_point(position);
                    bounding_box.extend(&world_position);
                }
            }
            let bounding_sphere = bounding_box.bounding_sphere();
            (bounding_box, bounding_sphere)
        };

        // Dynamic scene.
        let dyn_scene = {
            let materials = materials
                .iter()
                .map(|material| DynamicMaterial {
                    model: material.model,
                    base_color: material.base_color,
                    metallic: material.metallic,
                    roughness: material.roughness,
                    specular: material.specular,
                    specular_tint: material.specular_tint,
                })
                .collect();
            let textures = textures
                .iter()
                .map(|texture| match texture {
                    Texture::Scalar(s) => DynamicTexture::Scalar(*s),
                    Texture::Vector2(v) => DynamicTexture::Vector2(*v),
                    Texture::Vector3(v) => DynamicTexture::Vector3(*v),
                    Texture::Vector4(v) => DynamicTexture::Vector4(*v),
                    Texture::Image { .. } => DynamicTexture::Vector4([1.0, 1.0, 1.0, 1.0]),
                })
                .collect::<Vec<_>>();
            let default_textures = textures.clone();
            let mut replaced_textures = bitvec::bitvec!();
            replaced_textures.resize(textures.len(), false);
            DynamicScene {
                materials,
                textures,
                default_textures,
                replaced_textures,
            }
        };

        assert!(!cameras.is_empty());
        assert!(!meshes.is_empty());
        assert!(!materials.is_empty());
        assert!(!textures.is_empty());
        assert_eq!(materials.len(), dyn_scene.materials.len());
        assert_eq!(textures.len(), dyn_scene.textures.len());
        assert_eq!(dyn_scene.textures.len(), dyn_scene.replaced_textures.len());

        let scene = Scene {
            cameras,
            meshes,
            materials,
            textures,
            bounding_box,
            bounding_sphere,
        };

        Ok((scene, dyn_scene))
    }
}

//
// Importers
//

fn import_gltf_camera(
    gltf_camera: &gltf::Camera,
    gltf_node: &gltf::Node,
    cameras: &mut Vec<Camera>,
) -> Result<()> {
    use gltf::camera::Projection;

    // Name.
    let name = gltf_camera
        .name()
        .context("Camera must define a name")?
        .to_owned();

    // Transform.
    let transform = {
        let matrix = gltf_node.transform().matrix();
        unsafe { transmute(matrix) }
    };

    // Perspective projection.
    let (aspect_ratio, yfov, znear, zfar) = {
        let projection = gltf_camera.projection();
        let perspective = if let Projection::Perspective(perspective) = projection {
            perspective
        } else {
            bail!("Only perspective cameras are supported right now");
        };
        let aspect_ratio = perspective
            .aspect_ratio()
            .context("Cameras must define aspect ratio")?;
        let yfov = perspective.yfov();
        let znear = perspective.znear();
        let zfar = perspective.zfar().context("Cameras must define zfar")?;
        (aspect_ratio, yfov, znear, zfar)
    };

    // Append.
    cameras.push(Camera {
        name,
        transform,
        aspect_ratio,
        yfov,
        znear,
        zfar,
    });

    Ok(())
}

fn import_gltf_mesh(
    gltf_mesh: &gltf::Mesh,
    gltf_node: &gltf::Node,
    gltf_buffer_data: &[gltf::buffer::Data],
    gltf_image_data: &[gltf::image::Data],
    meshes: &mut Vec<Mesh>,
    materials: &mut Vec<Material>,
    textures: &mut Vec<Texture>,
) -> Result<()> {
    use gltf::mesh::Mode;

    // Primitive.
    let gltf_primitive = {
        ensure!(gltf_mesh.primitives().count() == 1);
        let gltf_primitive = gltf_mesh.primitives().next().unwrap();
        if !matches!(gltf_primitive.mode(), Mode::Triangles) {
            bail!("Only triangle meshes are supported right now");
        }
        gltf_primitive
    };

    // Name.
    let name = gltf_mesh
        .name()
        .context("Mesh must define a name")?
        .to_owned();

    // Transform.
    let transform = {
        let matrix = gltf_node.transform().matrix();
        unsafe { transmute(matrix) }
    };

    // Mesh attributes.
    let positions = import_gltf_positions(&gltf_primitive, gltf_buffer_data)?;
    let tex_coords = import_gltf_tex_coords(&gltf_primitive, gltf_buffer_data)?;
    let normals = import_gltf_normals(&gltf_primitive, gltf_buffer_data)?;
    let triangles = import_gltf_triangles(&gltf_primitive, gltf_buffer_data)?;
    let material = import_gltf_material(&gltf_primitive, gltf_image_data, materials, textures)?;

    // Append.
    meshes.push(Mesh {
        name,
        transform,
        positions,
        tex_coords,
        normals,
        triangles,
        material,
    });

    Ok(())
}

fn import_gltf_material(
    gltf_primitive: &gltf::Primitive,
    gltf_image_data: &[gltf::image::Data],
    materials: &mut Vec<Material>,
    textures: &mut Vec<Texture>,
) -> Result<u32> {
    use gltf::material::AlphaMode;

    let gltf_material = gltf_primitive.material();

    // Name.
    let name = gltf_material
        .name()
        .context("Material must define a name")?
        .to_owned();

    // Validate.
    ensure!(gltf_material.alpha_cutoff().is_none());
    ensure!(gltf_material.alpha_mode() == AlphaMode::Opaque);
    ensure!(gltf_material.double_sided() == false);
    ensure!(gltf_material.normal_texture().is_none());
    ensure!(gltf_material.occlusion_texture().is_none());
    ensure!(gltf_material.emissive_texture().is_none());
    let pbr = gltf_material.pbr_metallic_roughness();

    // Base color.
    let base_color = {
        let base_color = if let Some(base_color) = pbr.base_color_texture() {
            use gltf::image::Source;

            // Image.
            let image = {
                let texture = base_color.texture();
                let image = texture.source();
                let source = image.source();
                let view = if let Source::View { view, .. } = source {
                    view
                } else {
                    bail!("Source must be buffer view");
                };
                let buffer = view.buffer();
                let index = buffer.index();
                &gltf_image_data[index]
            };

            // Validate.
            let width = image.width;
            let height = image.height;
            let components = 4;
            let format = image.format;
            let pixels = &image.pixels;
            ensure!(width > 0 && width.is_power_of_two());
            ensure!(height > 0 && height.is_power_of_two());
            ensure!(format == gltf::image::Format::R8G8B8A8);
            ensure!((components * width * height) as usize == pixels.len());

            // Convert R8G8B8A8_UNORM -> R32G32B32A32_SFLOAT.
            let pixels = pixels
                .chunks_exact(components as usize)
                .flat_map(|chunk| {
                    // Todo: sRGB -> linear?
                    let r = f32::from(chunk[0]) / 255.0;
                    let g = f32::from(chunk[1]) / 255.0;
                    let b = f32::from(chunk[2]) / 255.0;
                    let a = f32::from(chunk[3]) / 255.0;
                    [r, g, b, a]
                })
                .collect();

            Texture::Image {
                width,
                height,
                components,
                pixels,
            }
        } else {
            Texture::Vector4(pbr.base_color_factor())
        };

        // Append.
        let texture_index = textures.len() as u32;
        textures.push(base_color);
        texture_index
    };

    // Roughness & metallic.
    let (metallic, roughness) = {
        let (metallic, roughness) = if pbr.metallic_roughness_texture().is_some() {
            todo!("Support metallic roughness textures");
        } else {
            (
                Texture::Scalar(pbr.metallic_factor()),
                Texture::Scalar(pbr.roughness_factor()),
            )
        };

        // Append.
        let metallic_index = textures.len() as u32;
        let roughness_index = textures.len() as u32 + 1;
        textures.push(metallic);
        textures.push(roughness);
        (metallic_index, roughness_index)
    };

    // Specular & specular tint.
    let (specular, specular_tint) = {
        // Todo: Check if Blender can export both metallic and specular.
        let specular = Texture::Scalar(0.5);
        let specular_tint = Texture::Scalar(0.0);
        let specular_index = textures.len() as u32;
        let specular_tint_index = textures.len() as u32 + 1;
        textures.push(specular);
        textures.push(specular_tint);
        (specular_index, specular_tint_index)
    };

    // Append.
    let material_index = materials.len() as u32;
    materials.push(Material {
        name,
        model: MaterialModel::Disney,
        base_color,
        metallic,
        roughness,
        specular,
        specular_tint,
    });

    Ok(material_index)
}

fn import_gltf_positions(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<Point3>> {
    use gltf::accessor::DataType;
    use gltf::accessor::Dimensions;
    use gltf::mesh::Semantic;

    // Accessor.
    let acc = gltf_primitive
        .attributes()
        .find_map(|(semantic, accessor)| {
            if semantic == Semantic::Positions {
                return Some(accessor);
            };
            None
        })
        .with_context(|| "Mesh is missing positions".to_string())?;

    // Validate.
    ensure!(acc.data_type() == DataType::F32);
    ensure!(acc.dimensions() == Dimensions::Vec3);
    ensure!(acc.size() == size_of::<Point3>());
    ensure!(acc.offset() == 0);
    ensure!(acc.normalized() == false);
    let view = acc.view().context("Accessor must have a buffer view")?;
    let offset = view.offset();
    let length = view.length();
    ensure!(view.stride().is_none());
    ensure!(length > 0);
    ensure!(length % size_of::<Point3>() == 0);

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    Ok(bytemuck::cast_slice(&buffer[offset..(offset + length)]).to_vec())
}

fn import_gltf_tex_coords(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<Point2>> {
    use gltf::accessor::DataType;
    use gltf::accessor::Dimensions;
    use gltf::mesh::Semantic;

    // Accessor.
    let acc = gltf_primitive
        .attributes()
        .find_map(|(semantic, accessor)| {
            if semantic == Semantic::TexCoords(0) {
                return Some(accessor);
            };
            None
        })
        .with_context(|| "Mesh is missing tex_coords".to_string())?;

    // Validate.
    ensure!(acc.data_type() == DataType::F32);
    ensure!(acc.dimensions() == Dimensions::Vec2);
    ensure!(acc.size() == size_of::<Point2>());
    ensure!(acc.offset() == 0);
    ensure!(acc.normalized() == false);
    let view = acc.view().context("Accessor must have a buffer view")?;
    let offset = view.offset();
    let length = view.length();
    ensure!(view.stride().is_none());
    ensure!(length > 0);
    ensure!(length % size_of::<Point2>() == 0);

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    Ok(bytemuck::cast_slice(&buffer[offset..(offset + length)]).to_vec())
}

fn import_gltf_normals(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<Normal>> {
    use gltf::accessor::DataType;
    use gltf::accessor::Dimensions;
    use gltf::mesh::Semantic;

    // Accessor.
    let acc = gltf_primitive
        .attributes()
        .find_map(|(semantic, accessor)| {
            if semantic == Semantic::Normals {
                return Some(accessor);
            };
            None
        })
        .with_context(|| "Mesh is missing normals".to_string())?;

    // Validate.
    ensure!(acc.data_type() == DataType::F32);
    ensure!(acc.dimensions() == Dimensions::Vec3);
    ensure!(acc.size() == size_of::<Normal>());
    ensure!(acc.offset() == 0);
    ensure!(acc.normalized() == false);
    let view = acc.view().context("Accessor must have a buffer view")?;
    let offset = view.offset();
    let length = view.length();
    ensure!(view.stride().is_none());
    ensure!(length > 0);
    ensure!(length % size_of::<Normal>() == 0);

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    Ok(bytemuck::cast_slice(&buffer[offset..(offset + length)]).to_vec())
}

fn import_gltf_triangles(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<Vec3u>> {
    use gltf::accessor::DataType;
    use gltf::accessor::Dimensions;

    // Accessor.
    let acc = gltf_primitive
        .indices()
        .with_context(|| "Mesh is missing triangles".to_string())?;

    // Validate.
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

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    let slice_u8 = &buffer[offset..(offset + length)];
    Ok(match acc.data_type() {
        DataType::U16 => {
            ensure!(length % 3 * size_of::<u16>() == 0);
            let slice_u16: &[u16] = bytemuck::cast_slice(slice_u8);
            slice_u16
                .chunks_exact(3)
                .map(|chunk| {
                    vector![
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
    })
}

//
// Camera
//

impl Camera {
    pub fn clip_from_view(&self) -> Perspective3 {
        Perspective3::new(self.aspect_ratio, self.yfov, self.znear, self.zfar)
    }

    pub fn world_from_view(&self) -> Mat4 {
        self.transform
    }

    pub fn position(&self) -> Point3 {
        let coords: Vec3 = self.transform.column(3).fixed_rows::<3>(0).into();
        Point3::from(coords)
    }
}

//
// Mesh
//

impl Mesh {
    pub fn triangle_count(&self) -> u32 {
        self.triangles.len() as u32
    }

    pub fn index_count(&self) -> u32 {
        (3 * self.triangles.len()) as u32
    }
}

//
// Material
//

impl MaterialModel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Diffuse => "diffuse",
            Self::Disney => "disney",
        }
    }
}

impl std::fmt::Display for MaterialModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl DynamicMaterial {
    pub fn texture(&self, field: MaterialField) -> u32 {
        match field {
            MaterialField::BaseColor => self.base_color,
            MaterialField::Metallic => self.metallic,
            MaterialField::Roughness => self.roughness,
            MaterialField::Specular => self.specular,
            MaterialField::SpecularTint => self.specular_tint,
        }
    }
}

//
// Texture
//

impl Texture {
    pub fn sample(&self, tex_coord: Point2) -> ColorRgba {
        match self {
            Self::Scalar(s) => ColorRgba::new(*s, 0.0, 0.0, 0.0),
            Self::Vector2(v) => ColorRgba::new(v[0], v[1], 0.0, 0.0),
            Self::Vector3(v) => ColorRgba::new(v[0], v[1], v[2], 0.0),
            Self::Vector4(v) => ColorRgba::new(v[0], v[1], v[2], v[3]),
            Self::Image {
                width,
                height,
                components,
                pixels,
            } => {
                let width = *width as usize;
                let height = *height as usize;
                let components = *components as usize;
                let x = tex_coord.x.clamp(0.0, 1.0);
                let y = tex_coord.y.clamp(0.0, 1.0);
                let x = f32::floor(x * width as f32) as usize;
                let y = f32::floor(y * height as f32) as usize;
                let x = usize::clamp(x, 0, width - 1);
                let y = usize::clamp(y, 0, height - 1);
                let offset = components * (y * width + x);
                let mut sample = [0.0_f32; 4];
                sample[..components].copy_from_slice(&pixels[offset..(components + offset)]);
                ColorRgba::new(sample[0], sample[1], sample[2], sample[3])
            }
        }
    }

    pub fn byte_count(&self) -> usize {
        match self {
            Self::Scalar(_) => size_of::<f32>(),
            Self::Vector2(_) => 2 * size_of::<f32>(),
            Self::Vector3(_) => 3 * size_of::<f32>(),
            Self::Vector4(_) => 4 * size_of::<f32>(),
            Self::Image { pixels, .. } => pixels.len() * size_of::<f32>(),
        }
    }
}

impl DynamicTexture {
    pub fn sample(&self) -> ColorRgba {
        match self {
            Self::Scalar(s) => ColorRgba::new(*s, 0.0, 0.0, 0.0),
            Self::Vector2(v) => ColorRgba::new(v[0], v[1], 0.0, 0.0),
            Self::Vector3(v) => ColorRgba::new(v[0], v[1], v[2], 0.0),
            Self::Vector4(v) => ColorRgba::new(v[0], v[1], v[2], v[3]),
        }
    }
}

struct DisplayArray<'a, const LEN: usize>(&'a [f32; LEN]);

impl<'a, const LEN: usize> std::fmt::Display for DisplayArray<'a, LEN> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, v) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            if let Some(precision) = f.precision() {
                write!(f, "{v:.precision$}")?;
            } else {
                write!(f, "{v}")?;
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for DynamicTexture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(precision) = f.precision() {
            match self {
                DynamicTexture::Scalar(s) => write!(f, "{s:.precision$}"),
                DynamicTexture::Vector2(v) => write!(f, "{:.precision$}", DisplayArray(v)),
                DynamicTexture::Vector3(v) => write!(f, "{:.precision$}", DisplayArray(v)),
                DynamicTexture::Vector4(v) => write!(f, "{:.precision$}", DisplayArray(v)),
            }
        } else {
            match self {
                DynamicTexture::Scalar(s) => write!(f, "{s}"),
                DynamicTexture::Vector2(v) => write!(f, "{}", DisplayArray(v)),
                DynamicTexture::Vector3(v) => write!(f, "{}", DisplayArray(v)),
                DynamicTexture::Vector4(v) => write!(f, "{}", DisplayArray(v)),
            }
        }
    }
}

pub fn dynamic_sample(
    scene: &Scene,
    dyn_scene: &DynamicScene,
    texture_index: u32,
    tex_coord: Point2,
) -> ColorRgba {
    let index = texture_index as usize;
    if dyn_scene.replaced_textures[index] {
        dyn_scene.textures[index].sample()
    } else {
        scene.textures[index].sample(tex_coord)
    }
}

pub fn dynamic_try_sample(dyn_scene: &DynamicScene, texture_index: u32) -> Option<ColorRgba> {
    let index = texture_index as usize;
    if dyn_scene.replaced_textures[index] {
        return Some(dyn_scene.textures[index].sample());
    }
    None
}

pub fn dynamic_model(dyn_scene: &DynamicScene, material_index: u32) -> MaterialModel {
    let index = material_index as usize;
    dyn_scene.materials[index].model
}

pub fn dynamic_material_by_name(
    scene: &Scene,
    dyn_scene: &DynamicScene,
    name: &str,
) -> Option<DynamicMaterial> {
    use itertools::Itertools;
    if let Some((material, _)) = scene.materials.iter().find_position(|m| m.name == name) {
        Some(dyn_scene.materials[material])
    } else {
        None
    }
}
