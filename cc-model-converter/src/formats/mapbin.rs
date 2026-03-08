use binread::{BinRead};
use super::cpt::CPTWithCompression;

pub struct MapBin {
    pub sections: Vec<Vec<u8>>,

    pub camera: Camera,
    pub paletteData: Vec<u8>,
    pub layers: LayersSection,
    pub walkMeshTriangles: WalkMeshTriangleSection,
    pub walkMeshVertices: WalkMeshVerticesSection,
}

impl BinRead for MapBin {
    type Args = ();
    
    fn read_options<R: std::io::Read + std::io::Seek>(reader: &mut R, _options: &binread::ReadOptions, _args: Self::Args) -> binread::BinResult<Self> {
        let sections = CPTWithCompression::read(reader)?.sections;
        Ok(MapBin {
            camera: Camera::read(&mut std::io::Cursor::new(&sections[0]))?,
            paletteData: sections[1].clone(),
            layers: LayersSection::read(&mut std::io::Cursor::new(&sections[7]))?,
            walkMeshTriangles: WalkMeshTriangleSection::read(&mut std::io::Cursor::new(&sections[4]))?,
            walkMeshVertices: WalkMeshVerticesSection::read(&mut std::io::Cursor::new(&sections[5]))?,

            sections,
        })
    }
}

#[derive(BinRead)]
pub struct MapBinModelHeader {
    pub modelByteLength: u32,
    pub textureBaseX: u16,
    pub textureBaseY: u16,
    pub textureClutX: u16,
    pub textureClutY: u16,
}

#[derive(BinRead)]
pub struct Camera {
    // 3x3 Matrix, row-major, fixed point 4.12
    pub m11: i16,
    pub m12: i16,
    pub m13: i16,
    pub m21: i16,
    pub m22: i16,
    pub m23: i16,
    pub m31: i16,
    pub m32: i16,
    pub m33: i16,
    
    // Translation component
    pub tx: i16,
    pub ty: i16,
    pub tz: i16,

    pub unknown1: i16, // Seems to correlate with needed zoom level, higher the further the camera is from the walk mesh
    pub maybe_xMax: i16,
    pub maybe_xMin: i16,
    pub maybe_yMax: i16,
    pub maybe_yMin: i16,
    pub unknown2: i16,

    // pub unknown1: [i16; 6],
    pub count: u32,
    #[br(count = count)]
    pub unknown3: Vec<u32>,
}

// From utunnels' room viewer Form1.cs

#[derive(BinRead)]
pub struct TileCommand {
    pub x: i16,
    pub y: i16,
    pub u: u16,
    pub v: u8,
    pub palette: u8,
    pub order: u8,
    pub mode: u8,
    pub z: u16,
}
#[derive(BinRead)]
pub struct LayersSection {
    pub numLayers: u32,
    #[br(count = numLayers + 1)]
    pub tileOffsets: Vec<u32>,
    #[br(count = tileOffsets[numLayers as usize])]
    pub tiles: Vec<TileCommand>,
}

#[derive(BinRead)]
pub struct WalkMeshTriangleSection {
    pub numTriangles: u32,
    #[br(count = numTriangles)]
    pub triangles: Vec<WalkMeshTriangle>
}
#[derive(BinRead)]
pub struct WalkMeshTriangle {
    pub index1: u16,
    pub index2: u16,
    pub index3: u16,
    pub adjacentFace1: i16, // Or -1 if no neighbor
    pub adjacentFace2: i16,
    pub adjacentFace3: i16,
    pub info: u16,
}

#[derive(BinRead)]
pub struct WalkMeshVerticesSection {
    pub numVerticesTimes8: u32,
    #[br(count = numVerticesTimes8 / 8)]
    pub vertices: Vec<WalkMeshVertex>
}
#[derive(BinRead)]
pub struct WalkMeshVertex {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub w: i16
}