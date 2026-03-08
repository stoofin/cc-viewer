use binread::{BinRead,FilePtr32, derive_binread};
use std::io::{SeekFrom};
use modular_bitfield::prelude::*;
use crate::{CurPos};
use super::Transform;

#[derive(BinRead, Debug)]
pub struct Vertex {
    x: i16,
    y: i16,
    z: i16,
    pub index: i16,
}

#[derive(BinRead, Debug)]
pub struct Normal {
    x: i8,
    y: i8,
    z: i8,
    w: i8,
}

#[derive(BinRead)]
pub struct VertexGroupOneJoint {
    pub numVertices: i16,
    pub joint: i16,
}

#[derive(BinRead)]
pub struct VertexGroupTwoJoints {
    pub numVertices: i16,
    pub unknown1: i16,
    pub jointB: i16, // NOTICE for some INSANE reason jointB is FIRST!!
    pub weightB: i16, // Un-normalized
    pub jointA: i16,
    pub weightA: i16, // Un-normalized
}

// 0x24 == 0b0010_0010
// Maybe reflects GPU render polygon command?
// https://psx-spx.consoledev.net/graphicsprocessingunitgpu/#gpu-render-polygon-commands
#[bitfield(bits = 16)]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(map = Self::from_bytes)]
#[br(assert(command == 1, "Face 'command' not 1??"))]
pub struct FaceType {
    // bitfield goes LSB first, so this is
    // [CCC V  Q T S R]
    // [001 V  Q T 0 0]

    pub MAYBE_isRawTexture: bool, // vs modulated
    pub MAYBE_isSemiTransparent: bool, // vs opaque

    pub isTextured: bool, // vs untextured
    pub isQuad: bool, // vs triangle
    // MAYBE_isGouraudShaded: bool, // vs flat shaded
    pub vertexColorPerVertex: bool, // vs one per vertex, UNTESTED by me so, unconfirmed

    pub MAYBE_command: B3, // Always 0b100 ??

    pub unknown1: B8,
}

impl FaceType {
    pub fn num_verts(&self) -> usize {
        if self.isQuad() { 4 } else { 3 }
    }
}

#[derive(BinRead, Debug, Clone, Copy)]
pub struct UV {
    pub u: u8,
    pub v: u8,
}
#[derive(BinRead, Debug, Clone)]
pub struct VertexColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub w: u8,
}

#[derive(BinRead, Debug, Clone)]
#[br(import(faceType: FaceType))]
pub struct Face {
    #[br(if(faceType.isTextured(), vec![UV { u: 0, v: 0 }; faceType.num_verts()]))]
    #[br(count = faceType.num_verts())]
    pub textureUV: Vec<UV>,

    #[br(if(!faceType.isTextured(), VertexColor { r: 255, g: 255, b: 255, w: 0 }))]
    pub vertexColor: VertexColor,

    #[br(count = faceType.num_verts())]
    pub indices: Vec<i16>,

    #[br(if(!faceType.isQuad() && !faceType.isTextured()))]
    pub extra: i16,
}

#[derive(BinRead, Debug)]
#[br(import(baseOffset: u64))]
pub struct FaceGroupChunk {
    pub faceType: FaceType,
    pub numFaces: i16,

    #[br(count = numFaces)]
    #[br(offset = baseOffset)]
    #[br(args(faceType))]
    pub faces: FilePtr32<Vec<Face>>,
}

#[derive(BinRead)]
#[br(import(baseOffset: u64))]
pub struct FaceGroup {
    pub faceOffset: u32,
    pub vertexOffset: u32,
    pub normalOffset: u32,

    pub numOneJointVertexGroups: u32,
    pub oneJointVerticesOffset: u32,
    #[br(count = numOneJointVertexGroups)]
    pub oneJointVertexGroups: Vec<VertexGroupOneJoint>,

    pub numTwoJointVertexGroups: u32,
    pub twoJointVerticesOffset: u32,
    #[br(count = numTwoJointVertexGroups)]
    pub twoJointVertexGroups: Vec<VertexGroupTwoJoints>,

    pub numChunks: u32,
    #[br(count = numChunks)]
    #[br(args(baseOffset + faceOffset as u64))]
    pub chunks: Vec<FaceGroupChunk>,

    #[br(calc = oneJointVertexGroups.iter().map(|g| g.numVertices as usize).sum())]
    pub numOneJointVertices: usize,
    #[br(calc = 2 * twoJointVertexGroups.iter().map(|g| g.numVertices as usize).sum::<usize>())]
    pub numTwoJointVertices: usize,

    #[br(count = numOneJointVertices)]
    #[br(offset = baseOffset + vertexOffset as u64 + oneJointVerticesOffset as u64)]
    #[br(restore_position)]
    #[br(seek_before(SeekFrom::Start(baseOffset + vertexOffset as u64 + oneJointVerticesOffset as u64)))]
    pub oneJointVertices: Vec<Vertex>,

    #[br(count = numTwoJointVertices)]
    #[br(restore_position)]
    #[br(seek_before(SeekFrom::Start(baseOffset + vertexOffset as u64 + twoJointVerticesOffset as u64)))]
    pub twoJointVertices: Vec<Vertex>,

    #[br(count = numOneJointVertices + numTwoJointVertices)]
    #[br(restore_position)]
    #[br(seek_before(SeekFrom::Start(baseOffset + normalOffset as u64)))]
    pub normals: Vec<Normal>
}

#[derive_binread]
pub struct FaceData {
    #[br(temp)]
    pub baseOffset: CurPos,

    pub numFaceGroups: u32,
    #[br(count = numFaceGroups)]
    #[br(offset = baseOffset.0)]
    #[br(args(baseOffset.0))]
    pub faceGroups: Vec<FilePtr32<FaceGroup>>,

    pub unknown1: u8,
    pub unknown2: u8,
    pub unknown3: u8,
    pub unknown4: u8,
    pub unknown5: u32,
}

#[derive(BinRead)]
pub struct Joint {
    pub parent: i32,

    pub transform: Transform,

    pub unknown1: i16,
    pub unknown2: i16,
}

#[derive(BinRead)]
pub struct TransformData {
    pub numJoints: u32,
    #[br(count = numJoints)]
    pub joints: Vec<Joint>,
}


#[derive_binread]
pub struct AttachmentPoint {
    x: i16,
    y: i16,
    z: i16,
    pub joint: i16,
}

pub const SEMI_TRANSPARENCY_MODE_MIX: u8 = 0;
pub const SEMI_TRANSPARENCY_MODE_ADD: u8 = 1;
pub const SEMI_TRANSPARENCY_MODE_SUBTRACT: u8 = 2;
pub const SEMI_TRANSPARENCY_MODE_ADD_ONE_FOURTH: u8 = 3;

#[bitfield(bits = 8)]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(map = Self::from_bytes)]
pub struct ColorInfoFlags {
    pub isInvisible: bool,
    pub isSemiTransparent: bool,
    pub semiTransparencyMode: B2,
    pub doubleSided: bool, // 0 for most things, 1 for Kid's skirt, 1 for adk_sp's skirt, maybe indicate double-sided, normals need to be flipped?
    // shinshi has all of these bits on, but which one means that the UVs need to be shifted down?
    pub unknown234: B3, // saw the bit in the middle turned on here for the jellyfish (kurage), unsure what it means
}

#[derive_binread]
pub struct ColorInfo {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub flags: ColorInfoFlags,
}

#[derive_binread]
pub struct WeaponTransform {
    pub transform: Transform,

    pub attachmentPoint: i16,
    pub unknown1: i16,
}

#[derive_binread]
pub struct Section3 {
    #[br(count = 32)]
    pub attachmentPoints: Vec<AttachmentPoint>,
    #[br(count = 16)]
    pub colorInfos: Vec<ColorInfo>,
    #[br(count = 2)]
    pub weaponTransforms: Vec<WeaponTransform>,
}

#[derive(BinRead, Debug)]
pub struct ModelHeader {
    pub numSections: u32,
    #[br(count = numSections)]
    pub sectionOffsets: Vec<u32>,

    pub modelLength: u32,
}


// Methods to access data

use glam::f32::{Vec3 as Vector3, Mat4 as Matrix4};

impl Vertex {
    pub fn position(&self) -> Vector3 {
        super::spatial(self.x, self.y, self.z)
    }
}
impl AttachmentPoint {
    pub fn position(&self) -> Vector3 {
        super::spatial(self.x, self.y, self.z)
    }
}
impl Normal {
    pub fn direction(&self) -> Vector3 {
        super::spatial_norm(self.x, self.y, self.z)
    }
}

pub fn applyTransform(v: &Vertex, m: &Matrix4) -> Vector3 {
    m.transform_point3(v.position())
}

pub fn applyTransformNormal(n: &Normal, m: &Matrix4) -> Vector3 {
    m.transform_vector3(n.direction())
}