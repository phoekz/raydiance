use super::*;

pub fn load_glb(glb: &[u8]) -> Result<(String, Vec<u8>)> {
    use std::io::{Cursor, Read};

    // Parse .glb.
    let (json, binary) = {
        macro_rules! read_u32 {
            ($rdr:ident) => {{
                let mut buf = [0; 4];
                $rdr.read_exact(&mut buf)?;
                u32::from_le_bytes(buf)
            }};
        }

        // Prepare reader.
        let mut rdr = Cursor::new(glb);

        // Header.
        {
            let magic = read_u32!(rdr);
            let version = read_u32!(rdr);
            let length = read_u32!(rdr);
            ensure!(magic == 0x46546C67);
            ensure!(version == 2);
            ensure!(length as usize == glb.len());
        }

        // Chunk (JSON).
        let json = {
            // Chunk header.
            let chunk_length = read_u32!(rdr);
            let chunk_type = read_u32!(rdr);
            ensure!(chunk_length > 0);
            ensure!(chunk_type == 0x4E4F534A);

            // Chunk data.
            let mut chunk_data = vec![0_u8; chunk_length as usize];
            rdr.read_exact(&mut chunk_data)?;
            String::from_utf8(chunk_data)?
        };

        // Chunk (Binary).
        let binary = {
            // Chunk header.
            let chunk_length = read_u32!(rdr);
            let chunk_type = read_u32!(rdr);
            ensure!(chunk_length > 0);
            ensure!(chunk_type == 0x004E4942);

            // Chunk data.
            let mut chunk_data = vec![0_u8; chunk_length as usize];
            rdr.read_exact(&mut chunk_data)?;
            chunk_data
        };

        (json, binary)
    };

    Ok((json, binary))
}

#[derive(Deserialize, Debug)]
#[serde(bound(deserialize = "'de: 'a"))]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Gltf<'a> {
    pub accessors: Vec<Accessor>,
    pub asset: Asset<'a>,
    pub buffer_views: Vec<BufferView>,
    pub buffers: Vec<Buffer>,
    pub cameras: Vec<Camera<'a>>,
    pub images: Vec<Image<'a>>,
    pub materials: Vec<Material<'a>>,
    pub meshes: Vec<Mesh<'a>>,
    pub nodes: Vec<Node<'a>>,
    pub samplers: Vec<Sampler>,
    pub scene: usize,
    pub scenes: Vec<Scene<'a>>,
    pub textures: Vec<Texture>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Id {
    pub index: usize,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "UPPERCASE")]
#[allow(dead_code)]
pub enum AccessorType {
    Scalar,
    Vec2,
    Vec3,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Accessor {
    pub buffer_view: usize,
    pub component_type: u32,
    pub count: usize,
    #[serde(rename = "type")]
    pub ty: AccessorType,
    pub min: Option<[f32; 3]>,
    pub max: Option<[f32; 3]>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Asset<'a> {
    pub copyright: &'a str,
    pub generator: &'a str,
    pub version: &'a str,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BufferView {
    pub buffer: usize,
    pub byte_length: usize,
    pub byte_offset: usize,
    pub target: Option<u32>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Buffer {
    pub byte_length: usize,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Camera<'a> {
    pub name: &'a str,
    #[serde(rename = "type")]
    pub ty: &'a str,
    pub perspective: Perspective,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Perspective {
    pub aspect_ratio: f32,
    pub yfov: f32,
    pub zfar: f32,
    pub znear: f32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Image<'a> {
    pub name: &'a str,
    pub mime_type: &'a str,
    pub buffer_view: usize,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Material<'a> {
    pub name: &'a str,
    pub pbr_metallic_roughness: PbrMetallicRoughness,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PbrMetallicRoughness {
    pub base_color_texture: Option<Id>,
    #[serde(default)]
    pub base_color_factor: BaseColorFactor,
    #[serde(default)]
    pub metallic_factor: MetallicFactor,
    #[serde(default)]
    pub roughness_factor: RoughnessFactor,
}

#[derive(Deserialize, Debug)]
pub struct BaseColorFactor(pub [f32; 4]);

impl Default for BaseColorFactor {
    fn default() -> Self {
        Self([1.0, 1.0, 1.0, 1.0])
    }
}

#[derive(Deserialize, Debug)]
pub struct MetallicFactor(pub f32);

impl Default for MetallicFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Deserialize, Debug)]
pub struct RoughnessFactor(pub f32);

impl Default for RoughnessFactor {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Mesh<'a> {
    pub name: &'a str,
    pub primitives: Vec<Primitive>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Primitive {
    pub attributes: Attributes,
    pub indices: usize,
    pub material: usize,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "UPPERCASE")]
#[allow(dead_code)]
pub struct Attributes {
    pub position: usize,
    pub texcoord_0: usize,
    pub normal: usize,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Node<'a> {
    pub name: &'a str,
    pub camera: Option<usize>,
    pub mesh: Option<usize>,
    pub translation: Option<[f32; 3]>,
    pub rotation: Option<[f32; 4]>,
    pub scale: Option<[f32; 3]>,
}

impl Node<'_> {
    pub fn transform(&self) -> Mat4 {
        let translation = self.translation.unwrap_or([0.0, 0.0, 0.0]);
        let rotation = self.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
        let scale = self.scale.unwrap_or([1.0, 1.0, 1.0]);

        let translation = na::Translation3::from(translation);
        let translation = translation.to_homogeneous();
        let rotation = na::Quaternion::new(rotation[3], rotation[0], rotation[1], rotation[2]);
        let rotation = na::UnitQuaternion::new_normalize(rotation);
        let rotation = rotation.to_homogeneous();
        let scale = na::Scale3::from(scale);
        let scale = scale.to_homogeneous();

        translation * rotation * scale
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Sampler {
    pub mag_filter: u32,
    pub min_filter: u32,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Scene<'a> {
    pub name: &'a str,
    pub nodes: Vec<usize>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct Texture {
    pub sampler: usize,
    pub source: usize,
}
