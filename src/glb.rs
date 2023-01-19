use super::*;

#[derive(Clone, Debug)]
pub struct Scene {
    pub cameras: Vec<Camera>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
}

#[derive(Clone, Debug)]
pub struct Camera {
    pub name: String,
    pub transform: na::Matrix4<f32>,
    pub aspect_ratio: f32,
    pub yfov: f32,
    pub znear: f32,
    pub zfar: f32,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub name: String,
    pub transform: na::Matrix4<f32>,
    pub positions: Vec<na::Point3<f32>>,
    pub tex_coords: Vec<na::Point2<f32>>,
    pub normals: Vec<na::UnitVector3<f32>>,
    pub triangles: Vec<na::Vector3<u32>>,
    pub material: u32,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub base_color: u32,
    pub metallic: u32,
    pub roughness: u32,
}

#[derive(Clone, Debug)]
pub enum TextureKind {
    BaseColor,
    Metallic,
    Roughness,
}

#[derive(Clone, Debug)]
pub struct Texture {
    pub kind: TextureKind,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<f32>,
}

//
// Scene
//

impl Scene {
    pub fn create(glb: &[u8]) -> Result<Self> {
        let mut scene = Scene {
            cameras: vec![],
            meshes: vec![],
            materials: vec![],
            textures: vec![],
        };

        // Import.
        let (gltf_document, gltf_buffer_data, gltf_image_data) =
            gltf::import_slice(glb).context("Importing gltf model")?;

        // Traverse.
        for gltf_scene in gltf_document.scenes() {
            for gltf_node in gltf_scene.nodes() {
                // Cameras.
                if let Some(gltf_camera) = gltf_node.camera() {
                    import_gltf_camera(&gltf_camera, &gltf_node, &mut scene)?;
                }
                // Meshes, materials, textures.
                else if let Some(gltf_mesh) = gltf_node.mesh() {
                    import_gltf_mesh(
                        &gltf_mesh,
                        &gltf_node,
                        &gltf_buffer_data,
                        &gltf_image_data,
                        &mut scene,
                    )?;
                }
            }
        }

        // Stats.
        {
            info!("Scene contains {} cameras", scene.cameras.len());
            info!("Scene contains {} meshes", scene.meshes.len());
            info!("Scene contains {} materials", scene.materials.len());
            info!("Scene contains {} textures", scene.textures.len());
        }

        Ok(scene)
    }
}

fn import_gltf_camera(
    gltf_camera: &gltf::Camera,
    gltf_node: &gltf::Node,
    scene: &mut Scene,
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
    scene.cameras.push(Camera {
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
    scene: &mut Scene,
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
    let material = import_gltf_material(&gltf_primitive, gltf_image_data, scene)?;

    // Append.
    scene.meshes.push(Mesh {
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
    scene: &mut Scene,
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
            let format = image.format;
            let pixels = &image.pixels;
            ensure!(width > 0 && width.is_power_of_two());
            ensure!(height > 0 && height.is_power_of_two());
            ensure!(format == gltf::image::Format::R8G8B8A8);
            ensure!((4 * width * height) as usize == pixels.len());

            // Convert R8G8B8A8_UNORM -> R32G32B32A32_SFLOAT.
            let pixels = pixels
                .chunks_exact(4)
                .flat_map(|chunk| {
                    // Todo: sRGB -> linear?
                    let r = f32::from(chunk[0]) / 255.0;
                    let g = f32::from(chunk[1]) / 255.0;
                    let b = f32::from(chunk[2]) / 255.0;
                    let a = f32::from(chunk[3]) / 255.0;
                    [r, g, b, a]
                })
                .collect();

            Texture {
                kind: TextureKind::BaseColor,
                width,
                height,
                pixels,
            }
        } else {
            let base_color_factor = pbr.base_color_factor();
            Texture {
                kind: TextureKind::BaseColor,
                width: 1,
                height: 1,
                pixels: vec![
                    base_color_factor[0],
                    base_color_factor[1],
                    base_color_factor[2],
                    1.0,
                ],
            }
        };

        // Append.
        let texture_index = scene.textures.len() as u32;
        scene.textures.push(base_color);
        texture_index
    };

    // Roughness & metallic.
    let (metallic, roughness) = {
        let (metallic, roughness) = if let Some(_) = pbr.metallic_roughness_texture() {
            todo!("Support metallic roughness textures");
        } else {
            let metallic_factor = pbr.metallic_factor();
            let roughness_factor = pbr.roughness_factor();
            (
                Texture {
                    kind: TextureKind::Metallic,
                    width: 1,
                    height: 1,
                    pixels: vec![metallic_factor],
                },
                Texture {
                    kind: TextureKind::Roughness,
                    width: 1,
                    height: 1,
                    pixels: vec![roughness_factor],
                },
            )
        };

        // Append.
        let metallic_index = scene.textures.len() as u32;
        let roughness_index = scene.textures.len() as u32 + 1;
        scene.textures.push(metallic);
        scene.textures.push(roughness);
        (metallic_index, roughness_index)
    };

    // Append.
    let material_index = scene.materials.len() as u32;
    scene.materials.push(Material {
        name,
        base_color,
        metallic,
        roughness,
    });

    Ok(material_index)
}

fn import_gltf_positions(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<na::Point3<f32>>> {
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
    ensure!(acc.size() == size_of::<na::Point3<f32>>());
    ensure!(acc.offset() == 0);
    ensure!(acc.normalized() == false);
    let view = acc.view().context("Accessor must have a buffer view")?;
    let offset = view.offset();
    let length = view.length();
    ensure!(view.stride().is_none());
    ensure!(length > 0);
    ensure!(length % size_of::<na::Point3<f32>>() == 0);

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    Ok(bytemuck::cast_slice(&buffer[offset..(offset + length)]).to_vec())
}

fn import_gltf_tex_coords(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<na::Point2<f32>>> {
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
    ensure!(acc.size() == size_of::<na::Point2<f32>>());
    ensure!(acc.offset() == 0);
    ensure!(acc.normalized() == false);
    let view = acc.view().context("Accessor must have a buffer view")?;
    let offset = view.offset();
    let length = view.length();
    ensure!(view.stride().is_none());
    ensure!(length > 0);
    ensure!(length % size_of::<na::Point2<f32>>() == 0);

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    Ok(bytemuck::cast_slice(&buffer[offset..(offset + length)]).to_vec())
}

fn import_gltf_normals(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<na::UnitVector3<f32>>> {
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
    ensure!(acc.size() == size_of::<na::UnitVector3<f32>>());
    ensure!(acc.offset() == 0);
    ensure!(acc.normalized() == false);
    let view = acc.view().context("Accessor must have a buffer view")?;
    let offset = view.offset();
    let length = view.length();
    ensure!(view.stride().is_none());
    ensure!(length > 0);
    ensure!(length % size_of::<na::UnitVector3<f32>>() == 0);

    // Reinterpret bytes.
    let buffer = &*gltf_buffer_data[view.buffer().index()];
    Ok(bytemuck::cast_slice(&buffer[offset..(offset + length)]).to_vec())
}

fn import_gltf_triangles(
    gltf_primitive: &gltf::Primitive,
    gltf_buffer_data: &[gltf::buffer::Data],
) -> Result<Vec<na::Vector3<u32>>> {
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
    })
}

//
// Camera
//

impl Camera {
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
// Texture
//

impl Texture {
    pub fn sample(&self, tex_coord: na::Point2<f32>) -> LinSrgba {
        // Pixel location.
        assert!(tex_coord.x >= 0.0);
        assert!(tex_coord.y >= 0.0);
        let x = f32::floor(tex_coord.x * self.width as f32) as usize;
        let y = f32::floor(tex_coord.y * self.height as f32) as usize;
        let x = usize::clamp(x, 0, (self.width - 1) as usize);
        let y = usize::clamp(y, 0, (self.height - 1) as usize);
        let offset = 4 * (y * self.width as usize + x);

        // Fetch.
        match self.kind {
            TextureKind::BaseColor => LinSrgba::new(
                self.pixels[offset],
                self.pixels[offset + 1],
                self.pixels[offset + 2],
                self.pixels[offset + 3],
            ),
            TextureKind::Metallic => {
                let metallic = self.pixels[offset];
                LinSrgba::new(metallic, metallic, metallic, metallic)
            }
            TextureKind::Roughness => {
                let roughness = self.pixels[offset];
                LinSrgba::new(roughness, roughness, roughness, roughness)
            }
        }
    }

    pub fn byte_count(&self) -> usize {
        self.pixels.len() * size_of::<f32>()
    }
}
