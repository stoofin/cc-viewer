use binread::{BinRead,FilePtr32};
use modular_bitfield::prelude::*;
use crate::formats::tim::TIM;
use crate::CurPos;
use crate::{FaceType, Vertex, UV, VertexColor};

// https://www.chronocompendium.com/Term/Mesh.html
#[bitfield(bits = 16)]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(map = Self::from_bytes)]
pub struct ClutInfo {
    pub clutX: B6,
    pub clutY: B10,
}

// https://web.archive.org/web/20160817104617/http://psx.rules.org/gpu.txt
#[bitfield(bits = 16)]
#[derive(BinRead, Debug, Clone, Copy)]
#[br(map = Self::from_bytes)]
pub struct TextureInfo {
    pub texturePageX: B4,
    pub texturePageY: B1,
    pub transparencyMode: B2,
    pub mode: B2,
    pub dither: B1,
    pub drawToDisplayAreaAllowed: B1,
    applyMaskBitToDrawnPixels: B1,
    doNotDrawToPixelsWithSetMaskBit: B1,
    unknown: B3,
}

// Like polygon packet data? https://problemkaputt.de/psx-spx.htm#cdromfilevideo3dgraphicstmdpmdtodhmdrsdsony
#[derive(BinRead, Clone)]
#[br(import(faceType: FaceType))]
pub(crate) struct WeaponFaceUVs {
    pub uv1: UV,
    pub clutInfo: ClutInfo,
    pub uv2: UV,
    pub textureInfo: TextureInfo,
    pub uv3: UV,
    #[br(if(faceType.num_verts() == 4))]
    pub uv4: Option<UV>,
}

#[derive(BinRead)]
#[br(import(faceType: FaceType))]
pub(crate) struct WeaponFace {
    #[br(count = if faceType.vertexColorPerVertex() { faceType.num_verts() } else { 1 })]
    pub vertexColors: Vec<VertexColor>,

    #[br(args(faceType))]
    #[br(if(faceType.isTextured()))]
    pub uvs: Option<WeaponFaceUVs>,

    #[br(count = faceType.num_verts())]
    pub indices: Vec<u16>,

    #[br(if(!faceType.isQuad() && !faceType.isTextured()))]
    pub alignmentPadding: i16,
}

#[derive(BinRead)]
#[br(import(baseOffset: u64))]
pub struct WeaponFaceChunk {
    pub faceType: FaceType,
    pub numFaces: u16,

    #[br(offset = baseOffset)]
    #[br(count = numFaces)]
    #[br(args(faceType))]
    pub faces: FilePtr32<Vec<WeaponFace>>,
}

#[derive(BinRead)]
pub struct WeaponGeometry {
    baseOffset: CurPos,

    pub _unknown1: u32,
    pub numVertices: u16,
    pub _unknown2: u16,
    #[br(offset = baseOffset.0)]
    #[br(count = numVertices)]
    pub vertices: FilePtr32<Vec<Vertex>>,
    pub facePoolOffset: u32,
    pub _unknown3: u32,

    pub numGroups: u32,
    #[br(count = numGroups)]
    #[br(args(baseOffset.0))]
    pub groups: Vec<WeaponFaceChunk>,

    facePoolCurPos: CurPos,
    #[br(assert(facePoolCurPos.0 - baseOffset.0 == facePoolOffset as u64, "Where the faces at?"))]
    empty: (),
}

#[derive(BinRead)]
pub struct WeaponModel {
    pub tim: FilePtr32<TIM>,
    pub geo: FilePtr32<WeaponGeometry>,
}

#[derive(BinRead)]
pub struct EndOfWeaponPadding {
    pub magic_padding_number: u32,
    #[br(assert(magic_padding_number == 0x77777720, "No weapon padding??"))]
    empty: (),
}

#[derive(BinRead)]
pub struct WeapBin {
    pub num_objects: u32,
    #[br(count = num_objects / 2 - 1)]
    pub weapons: Vec<WeaponModel>,
    pub endOfCptOffset: u32,
}