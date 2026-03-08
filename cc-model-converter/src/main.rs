/*
Port of utunnels' Chrono Cross model import script for blender 2.49 (import_ccmodel2.3.1.py) to rust
Supports conversion to OBJ (without animation) and glTF
*/

#![allow(non_snake_case, dead_code)]

use std::collections::{HashMap};

use binread::{BinRead, BinResult, ReadOptions};
use std::{f32::INFINITY, io::{Read, Seek, SeekFrom}};
use jzon::{object};

mod gltf;
mod formats;
use formats::{ltd, ltc, tim, weapbin, prd, camera_path, meshinstance, cpt, mapbin, mapctd, animated_model::*, animations::*, Transform};

use glam::f32::{Vec2 as Vector2, Vec3 as Vector3, Vec4 as Vector4, vec2, vec3, vec4, Mat4 as Matrix4, Mat3 as Matrix3, Quat as Quaternion};

use anyhow::Result;

use crate::formats::chunkctd;
type Cursor<'a> = binread::io::Cursor<&'a [u8]>;

// https://github.com/jam1garner/binread/issues/33#issuecomment-757585179
pub struct CurPos(pub u64);
impl BinRead for CurPos {
    type Args = ();

    fn read_options<R: Read + Seek>(reader: &mut R, _ro: &ReadOptions, _args: Self::Args) -> BinResult<Self> {
        Ok(CurPos(reader.stream_position()?))
    }
}


// =================================================

struct Model {
    faceData: FaceData,
    transformData: TransformData,
    section3: Section3,
    animationData: AnimationData,
}

#[derive(Debug)]
enum ExtraIndexData {
    FourBits(u8),
    ThreeBits(u8),
}

fn calcIndices(fg: &FaceGroup) -> (Vec<u16>, Vec<ExtraIndexData>) {
    let mut result = vec![];
    let mut resultExtra = vec![];
    for chunk in fg.chunks.iter() {
        let mut history: Vec<u16> = vec![];
        for face in chunk.faces.iter() {
            for (i, data) in face.indices.iter().cloned().enumerate() {
                let (indexPointer, extraData) = if chunk.faceType.isTextured() && i % 4 < 2 { 
                    (data >> 4,  ExtraIndexData::FourBits((data & 0b1111) as u8))
                } else {
                    (data >> 3,  ExtraIndexData::ThreeBits((data & 0b111) as u8))
                };
                let index = if indexPointer < 0 {
                    history[(history.len() as isize + indexPointer as isize) as usize]
                } else {
                    indexPointer as u16
                };
                history.push(index);
                result.push(index);
                resultExtra.push(extraData);
            }
            if face.indices.len() == 3 {
                history.push(0);
            }
        }
    }
    (result, resultExtra)
}

fn readFaceData(cursor: &mut Cursor, offset: u32) -> Result<FaceData> {
    cursor.seek(SeekFrom::Start(offset as u64))?;
    let mut data = FaceData::read(cursor)?;

    // Fix up face index order
    for group in &mut data.faceGroups {
        for chunk in &mut group.chunks {
            for face in chunk.faces.iter_mut() {
                if let &[a, b, c] = &face.indices[..] {
                    face.indices = vec![b, c, a]; // For some reason, the triangle indices need to be shuffled like this
                }
            }
        }
    }

    // for group in &data.faceGroups {
    //     eprintln!("Num chunks: {}", group.numChunks);
    //     dbg!(group.faceOffset);
    //     dbg!(group.oneJointVerticesOffset);
    //     dbg!(group.numOneJointVertexGroups);
    //     dbg!(group.twoJointVerticesOffset);
    //     dbg!(group.numTwoJointVertexGroups);
    //     dbg!(group.oneJointVertices.len());
    //     dbg!(group.twoJointVertices.len());
    //     for chunk in &group.chunks {
    //         // eprintln!("face group: {:?}", chunk);
    //         dbg!(&chunk.faces[0]);
    //     }
    //     eprintln!("{:?}", calcIndices(group).0);
    // }

    // dbg!(data.faceGroups[1].twoJointVertices[75].index);

    Ok(data)
}

fn readModel(cursor: &mut Cursor) -> Result<Model> {
    let header = ModelHeader::read(cursor)?;

    eprintln!("Model Header: {:?}", header);

    let faceDataOffset = header.sectionOffsets[0];
    let faceData = readFaceData(cursor, faceDataOffset)?;

    cursor.seek(SeekFrom::Start(header.sectionOffsets[1] as u64))?;
    let transformData = TransformData::read(cursor)?;

    cursor.seek(SeekFrom::Start(header.sectionOffsets[2] as u64))?;
    let section3 = Section3::read(cursor)?;

    cursor.seek(SeekFrom::Start(header.sectionOffsets[3] as u64))?;
    let animationData = AnimationData::read_args(cursor, (transformData.numJoints,))?;

    dbg!(transformData.numJoints);
    dbg!(animationData.numAnimations);


    Ok(Model { faceData, transformData, section3, animationData })
}

#[derive(Debug)]
enum AnimatedJointType {
    Translation(Vec<Vector3>),
    Rotation(Vec<Quaternion>),
}
#[derive(Debug)]
struct AnimatedChannel {
    joint: usize,
    propValues: AnimatedJointType,
}
struct AnimationBuffer {
    numFrames: usize,
    times: Vec<f32>,
    channels: Vec<AnimatedChannel>,
}
fn build_animation_buffer(joints: &[Joint], anim: &Animation, _animIndex: usize) -> AnimationBuffer {
    let bitflagBytes: &[u8] = bytemuck::cast_slice(&anim.jointBitFlags[..]);
    let checkBit = |i| {
        let (byte, bit) = (i / 8, i % 8);
        (bitflagBytes[byte] >> bit) & 1 != 0
    };
    let rotationEnabled = |jointIndex| checkBit(jointIndex as usize * 2 + 0);
    let translationEnabled = |jointIndex| checkBit(jointIndex as usize * 2 + 1);

    // Accumulate all translations/rotations for all joints
    let mut translations: Vec<Vec<Vector3>> = vec![vec![vec3(0.0, 0.0, 0.0)]; joints.len()];
    let mut rotations: Vec<Vec<Quaternion>> = vec![vec![Quaternion::IDENTITY]; joints.len()];
    let mut hasInitialTranslation: Vec<bool> = vec![false; joints.len()];
    let mut hasInitialRotation: Vec<bool> = vec![false; joints.len()];

    // Overwrite initial transforms
    for (jointIndex, (Joint { transform: baseTransform, .. }, initialAnimTransform)) in joints.iter().zip(&anim.initialTransforms).enumerate() {
        rotations[jointIndex][0] = combinedTransformsAsQuaternion(baseTransform, initialAnimTransform);
        translations[jointIndex][0] = baseTransform.translation() + initialAnimTransform.translation();
        hasInitialRotation[jointIndex] = initialAnimTransform.has_nonzero_rotation();
        hasInitialTranslation[jointIndex] = initialAnimTransform.has_nonzero_translation();
    }
    // For every frame, iterate all joints and check the bitflags for each
    for keyframe in &anim.keyframes {
        let mut keyframeIter = keyframe.jointKeyframes.iter();
        // let mut dbgKeyframeOffset = 0;
        for (jointIndex, Joint { transform: baseTransform, .. }) in joints.iter().enumerate() {
            if rotationEnabled(jointIndex) { // Is this joint's rotation animated
                let rotation = keyframeIter.next().unwrap();
                let combinedQuaternion = combinedRotationAsQuaternion(baseTransform, rotation);
                rotations[jointIndex].push(combinedQuaternion);
            }
            if translationEnabled(jointIndex) { // Is this joint's translation animated
                let translation = keyframeIter.next().unwrap();
                let combinedTranslation = baseTransform.translation() + translation.as_translation();
                translations[jointIndex].push(combinedTranslation);
            }
        }
        assert!(keyframeIter.next().is_none()); 
    }

    let mut channels = vec![];
    for (jointIndex, jointTranslations) in translations.into_iter().enumerate() {
        if translationEnabled(jointIndex) || hasInitialTranslation[jointIndex] {
            channels.push(AnimatedChannel {
                joint: jointIndex,
                propValues: AnimatedJointType::Translation(jointTranslations),
            })
        }
    }
    for (jointIndex, jointRotations) in rotations.into_iter().enumerate() {
        if rotationEnabled(jointIndex) || hasInitialRotation[jointIndex] {
            channels.push(AnimatedChannel {
                joint: jointIndex,
                propValues: AnimatedJointType::Rotation(jointRotations),
            })
        }
    }

    AnimationBuffer {
        numFrames: anim.numFrames as usize,
        times: (0..anim.numFrames).into_iter().map(|n| n as f32 * 1.0 / 15.0).collect(),
        channels
    }
}

const HELP_STR: &'static str = r#"
Chrono Cross model (.mdl, .obj) converter, emits converted gltf to stdout

Usage: cc-model-converter [OPTIONS] model

Options:
    --zip zipfile
        Look for the file inside the specified zip file (e.g. cdrom.dat)
    --add-bin-anims bin_path
        Add animations from the specified bin file to this model (e.g. ex/selju.bin)
    --add-prd-anims prd_path subfilename
        Add animations from the specified subfile in the specified prd file to this model (e.g. someeffect.prd tech)
    --type={weapon|model|room|tim|roomctd|ltd|ltc|mesh|prd|effectctd}
        The type to parse the file as, if omitted it will guess based on file extension
    -h, --help
        Prints this message
"#;

struct FilesSource {
    zip_archive: Option<(Vec<u8>, rawzip::ZipArchive<rawzip::FileReader>)>,
}
impl FilesSource {
    fn read_file<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<Vec<u8>> {
        if let Some((buffer, rawzipArchive)) = &mut self.zip_archive {
            let mut entries_iterator = rawzipArchive.entries(buffer);
            while let Some(entry) = entries_iterator.next_entry()? {
                let entry_path = entry.file_path().try_normalize()?;
                if AsRef::<std::path::Path>::as_ref(entry_path.as_str()) == path.as_ref() {
                    let mut result_buf = vec![];
                    let local_entry = rawzipArchive.get_entry(entry.wayfinder())?;
                    match entry.compression_method() {
                        rawzip::CompressionMethod::Store => {
                            let mut reader = local_entry.verifying_reader(local_entry.reader());
                            std::io::copy(&mut reader, &mut result_buf)?;
                        },
                        rawzip::CompressionMethod::Deflate => {
                            let bufread = std::io::BufReader::new(local_entry.reader());
                            let decompressor = flate2::bufread::DeflateDecoder::new(bufread);
                            let mut reader = local_entry.verifying_reader(decompressor);
                            std::io::copy(&mut reader, &mut result_buf)?;
                        },
                        _ => unimplemented!(),
                    }
                    return Ok(result_buf);
                }
            }
            Err(anyhow::anyhow!("Zip entry not found: {:?}", path.as_ref()))
        } else {
            Ok(std::fs::read(path)?)
        }
    }
    fn is_zip(&self) -> bool {
        self.zip_archive.is_some()
    }
}

fn path_stem(filename: &(impl AsRef<std::path::Path> + ?Sized)) -> &str {
    filename.as_ref().file_stem().unwrap().to_str().unwrap()
}

struct TextureInfo {
    size: (usize, usize),
    relativeName: Option<String>,
    pngBuffer: Option<Vec<u8>>,
}
fn find_texture(source: &mut FilesSource, basePath: impl AsRef<std::path::Path>) -> Result<TextureInfo> {
    let mut texturePath = std::path::PathBuf::from(basePath.as_ref());

    texturePath.set_extension("png");
    eprintln!("Checking for texture at: {:?}", texturePath);
    if let Ok(texturePng) = source.read_file(&texturePath) {
        let reader = png::Decoder::new(Cursor::new(&texturePng)).read_info().unwrap();
        return Ok(TextureInfo {
            size:  (reader.info().width as usize, reader.info().height as usize),
            relativeName: if source.is_zip() { None } else { Some(texturePath.file_name().unwrap().to_str().unwrap().to_string()) },
            pngBuffer: Some(texturePng),
        });
    }
    texturePath.set_extension("ltc");
    eprintln!("Checking for texture at {:?}", texturePath);
    if let Ok(ltcBin) = source.read_file(&texturePath) {
        if let Ok(ltcObj) = ltc::LTC::read(&mut Cursor::new(&ltcBin)) {
            std::assert!(ltcObj.images.len() == 1 || ltcObj.images.len() == 2); // Can be two textures, second one is closed eye texture for blinking
            std::assert_eq!(ltcObj.images[0].images.len(), 1); // There should only be one texture in an ltd file, right?
            let image = ltcObj.images[0].to_single_rgba().unwrap();
            return Ok(TextureInfo {
                size: (image.width as usize, image.height as usize),
                relativeName: None,
                pngBuffer: Some(image.to_png())
            });
        }
    }
    texturePath.set_extension("ltd");
    eprintln!("Checking for texture at {:?}", texturePath);
    if let Ok(ltdBin) = source.read_file(&texturePath) {
        if let Ok(ltdTex) = ltd::LTD::read(&mut Cursor::new(&ltdBin)) {

            let texture_data = ltdTex.to_single_rgba();
            // Some have more than one, e.g. battle/monster/kurage/kurage.ltd
            eprintln!("Number of textures found in ltd: {}", ltdTex.images.len());
            // std::assert_eq!(pngs.len(), 1); // There should only be one texture in an ltd file, right?
            let image = texture_data.unwrap();
            return Ok(TextureInfo {
                size: (image.width as usize, image.height as usize),
                relativeName: None,
                pngBuffer: Some(image.to_png())
            });
        }
    }
    texturePath.set_extension("tim");
    eprintln!("Checking for texture at {:?}", texturePath);
    if let Ok(timBin) = source.read_file(&texturePath) {
        if let Ok(timTex) = tim::TIM::read(&mut Cursor::new(&timBin)) {
            let image = timTex.to_rgba();
            return Ok(TextureInfo {
                size: (image.width as usize, image.height as usize),
                relativeName: None,
                pngBuffer: Some(image.to_png())
            });
        }
    }

    Err(anyhow::anyhow!("Couldn't find a texture"))
}

fn rgbaImageToTextureInfo(rgbaImage: ltd::RGBAImage) -> TextureInfo {
    TextureInfo {
        relativeName: None,
        size: (rgbaImage.width as usize, rgbaImage.height as usize),
        pngBuffer: Some(rgbaImage.to_png())
    }
}

fn make_black_texture(width: u32, height: u32) -> TextureInfo {
    let mut pixels: Vec<u8> = vec![];
    for _y in 0..height {
        for _x in 0..width {
            pixels.extend([0, 0, 0, 255]);
        }
    }
    rgbaImageToTextureInfo(ltd::RGBAImage { width, height, pixels })
}

fn make_magenta_checkerboard_image(width: u32, height: u32, check_size: u32) -> ltd::RGBAImage {
    let mut pixels: Vec<u8> = vec![];
    for y in 0..height {
        for x in 0..width {
            if (x / check_size) % 2 != (y / check_size) % 2 {
                pixels.extend([255, 0, 255, 255]);
            } else {
                pixels.extend([0, 0, 0, 255]);
            }
        }
    }
    ltd::RGBAImage { width, height, pixels }
}

fn make_magenta_checkerboard(width: u32, height: u32, check_size: u32) -> TextureInfo {
    rgbaImageToTextureInfo(make_magenta_checkerboard_image(width, height, check_size))
}

impl TextureInfo {
    fn uri(&self) -> String {
        if let Some(relativeName) = &self.relativeName {
            relativeName.clone()
        } else if let Some(buffer) = &self.pngBuffer {
            bytesToMimeURI("image/png",buffer)
        } else {
            "no-texture".to_string()
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}
impl Rect {
    fn end(&self) -> (u16, u16) {
        (self.x + self.width, self.y + self.height)
    }
    fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x && other.end().0 <= self.end().0 && other.y >= self.y && other.end().1 <= self.end().1
    }
    fn union(&self, other: &Rect) -> Rect {
        use std::cmp::{min, max};
        let startX = min(self.x, other.x);
        let startY = min(self.y, other.y);
        let endX = max(self.end().0, other.end().0);
        let endY = max(self.end().1, other.end().1);
        Rect {
            x: startX,
            y: startY,
            width: endX - startX,
            height: endY - startY,
        }
    }
}

const BYTE: u64 = 5120;
const UNSIGNED_BYTE: u64 = 5121;
const SHORT: u64 = 5122;
const UNSIGNED_SHORT: u64 = 5123;
const UNSIGNED_INT: u64 = 5125;
const FLOAT: u64 = 5126;

const ARRAY_BUFFER: u64 = 34962;
const ELEMENT_ARRAY_BUFFER: u64 = 34963;

const NEAREST: u64 = 9728;
const LINEAR: u64 = 9729;
const NEAREST_MIPMAP_NEAREST: u64 = 9984;
const LINEAR_MIPMAP_NEAREST: u64 = 9985;
const NEAREST_MIPMAP_LINEAR: u64 = 9986;
const LINEAR_MIPMAP_LINEAR: u64 = 9987;

const CLAMP_TO_EDGE: u64 = 33071;
const MIRRORED_REPEAT: u64 = 33648;
const REPEAT: u64 = 10497;

#[derive(Copy, Clone)]
enum OutputType {
    Obj, Gltf
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
struct TexturePageKey {
    x: u8,
    y: u8,
    clutX: u16,
    clutY: u16,
    mode: u8, // 0 == 4bpp, 1 == 8bpp
}
struct TexturePage {
    occupiedRect: Rect,
    textureInfo: TextureInfo,
}

const VRAM_WIDTH: usize = 1024;
const VRAM_HEIGHT: usize = 512;

struct Vram {
    vram: Vec<u16>,
    images: HashMap<(TexturePageKey, bool), VramTexturePage>,
}

#[derive(Copy, Clone)]
struct VramTexturePage {
    rect: Rect,
    image: usize,
}

impl Vram {
    pub fn new() -> Vram {
        let size = VRAM_WIDTH * VRAM_HEIGHT;
        let mut vram = Vec::with_capacity(size);
        // Initialize with a checkerboard that checkerboards as 4bpp, 8bpp, 16bpp
        for _ in 0..VRAM_HEIGHT / 32 {
            for _ in 0..16 {
                for _ in 0..VRAM_WIDTH / 16 {
                    vram.extend([0xffff; 8]);
                    vram.extend([0x0421; 8]);
                }
            }
            for _ in 0..16 {
                for _ in 0..VRAM_WIDTH / 16 {
                    vram.extend([0x0421; 8]);
                    vram.extend([0xffff; 8]);
                }
            }
        }
        Vram {
            vram,
            images: HashMap::new(),
        }
    }
    pub fn get_texture_page_rgba(&self, key: TexturePageKey, twoPagesWide: bool) -> ltd::RGBAImage {
        let clutBase = key.clutY as usize * VRAM_WIDTH + key.clutX as usize;
        let clutWidth = match key.mode {
            0 => 16,
            1 => 256,
            2 => 0,
            other => { eprintln!("Unhandled mode {}", other); unimplemented!(); }
        };
        let clut = &self.vram[clutBase..clutBase + clutWidth];

        let clutPixels: Vec<[u8; 4]> = clut.iter().map(|&p| ltd::psx16_to_rgba8888(p)).collect();

        let baseX = key.x as usize * 64;
        let baseY = key.y as usize * 256;

        let nPagesWide: usize = if twoPagesWide { 2 } else { 1 };

        let width: usize = match key.mode {
            0 => 256 * nPagesWide,
            1 => 128 * nPagesWide,
            2 => 64 * nPagesWide,
            _ => unimplemented!(),
        };
        let height: usize = 256;

        let mut pixels = vec![];
        match key.mode {
            0 => {
                for y in 0..height {
                    let vramStart = (baseY + y) * VRAM_WIDTH + baseX;
                    let indices = &self.vram[vramStart..vramStart + 64 * nPagesWide];
                    for i in indices {
                        pixels.extend(clutPixels[((i >> 0) & 0xf) as usize]);
                        pixels.extend(clutPixels[((i >> 4) & 0xf) as usize]);
                        pixels.extend(clutPixels[((i >> 8) & 0xf) as usize]);
                        pixels.extend(clutPixels[((i >> 12) & 0xf) as usize]);
                    }
                }
            },
            1 => {
                for y in 0..height {
                    let vramStart = (baseY + y) * VRAM_WIDTH + baseX;
                    let indices = &self.vram[vramStart..vramStart + 64 * nPagesWide];
                    for i in indices {
                        pixels.extend(clutPixels[((i >> 0) & 0xff) as usize]);
                        pixels.extend(clutPixels[((i >> 8) & 0xff) as usize]);
                    }
                }
            },
            2 => {
                for y in 0..height {
                    let vramStart = (baseY + y) * VRAM_WIDTH + baseX;
                    let ps = &self.vram[vramStart..vramStart + 64 * nPagesWide];
                    for &p in ps {
                        pixels.extend(ltd::psx16_to_rgba8888(p));
                    }
                }
            },
            _ => unimplemented!(),
        }

        std::assert_eq!(width * height * 4, pixels.len());

        ltd::RGBAImage {
            width: width as u32,
            height: height as u32,
            pixels,
        }
    }
    pub fn get_texture_page(&mut self, key: TexturePageKey, twoPagesWide: bool, doc: &mut gltf::Gltf) -> VramTexturePage {
        if let Some(&result) = self.images.get(&(key, twoPagesWide)) {
            return result;
        }

        let image = self.get_texture_page_rgba(key, twoPagesWide);

        let imageGltfIndex = doc.add("images", object! {
            "name": format!("TexturePage@{},{} Mode={} Clut@{},{}", key.x, key.y, key.mode, key.clutX, key.clutY),
            "uri": (bytesToMimeURI("image/png", &image.to_png())),
        });
        let page = VramTexturePage { rect: Rect { x: 0, y: 0, width: image.width as u16, height: image.height as u16 }, image: imageGltfIndex };

        self.images.insert((key, twoPagesWide), page);
        page
    }
    fn add_u16s(&mut self, x: usize, y: usize, data: &[u16]) {
        let start = y * VRAM_WIDTH + x;
        self.vram[start..start + data.len()].copy_from_slice(data);
    }
    fn add_tim(&mut self, tim: &tim::TIM) {
        fn add_pixels_to_vram(vram: &mut Vram, pixels: &tim::PixelsData) {
            for y in 0..pixels.height as usize {
                let srcStart = y * pixels.width as usize;
                vram.add_u16s(pixels.x as usize, pixels.y as usize + y, &pixels.pixels[srcStart..srcStart + pixels.width as usize]);
            }
        }
        if let Some(clut) = &tim.clut {
            add_pixels_to_vram(self, clut);
        }
        add_pixels_to_vram(self, &tim.pixels);
    }
    fn add_ltd(&mut self, ltd: &ltd::LTD) {
        for y in 0..ltd.clut.height as usize {
            let srcStart = y * ltd.clut.width as usize;
            self.add_u16s(ltd.clut.x as usize, ltd.clut.y as usize + y, &ltd.clut.clut[srcStart..srcStart + ltd.clut.width as usize]);
        }
        for img in ltd.images.iter() {
            let pixelu16s: &[u16] = bytemuck::cast_slice(&img.pixels);
            let baseX = (img.texturePageX + img.x) as usize;
            let baseY = (img.texturePageY + img.y) as usize;
            for y in 0..img.height as usize {
                let srcStart = y * img.width as usize;
                self.add_u16s(baseX, baseY + y, &pixelu16s[srcStart..srcStart + img.width as usize]);
            }
        }
    }
    fn add_ctd(&mut self, ctd: &chunkctd::ChunkCTD) {
        for section in ctd.sections.iter() {
            let baseX = section.header.x1 + section.header.x2;
            let baseY = section.header.y1 + section.header.y2;
            let width = section.header.vramWidth;
            let mut row = 0;
            for (sector, &numRows) in section.sectors.iter().zip(section.header.rowsInSector.iter()) {
                for sectorRow in 0..numRows {
                    let srcStart = sectorRow as usize * width as usize;
                    self.add_u16s(baseX as usize, baseY as usize + row, &sector.data[srcStart..srcStart + width as usize]);
                    row += 1;
                }
            }
        }
    }
}

fn add_weapon_mesh(doc: &mut gltf::Gltf, name: &str, vram: &mut Vram, geo: &weapbin::WeaponGeometry) -> usize {
    #[derive(Clone, Hash, PartialEq, Eq)]
    struct MeshTextureInfo {
        // Don't consider clut info, chrono cross almost never swaps cluts for the same texture (exception being element field ui in battle iirc)
        whichTexture: TexturePageKey,
        transparencyMode: u8,
    }
    // Different faces can use different texture pages, especially for battlefield meshes this is true.
    let mut facesGroupedByTexture = std::collections::HashMap::<Option<MeshTextureInfo>, Vec<(FaceType, &weapbin::WeaponFace)>>::new();
    for g in &geo.groups {
        for f in g.faces.iter() {
            if let Some(uvs) = &f.uvs {
                facesGroupedByTexture.entry(Some(MeshTextureInfo {
                    whichTexture: TexturePageKey {
                        x: uvs.textureInfo.texturePageX() as u8,
                        y: uvs.textureInfo.texturePageY() as u8,
                        clutX: (uvs.clutInfo.clutX() as u16 + 255) / 256 * 256,
                        clutY: uvs.clutInfo.clutY() as u16,
                        mode: uvs.textureInfo.mode() as u8,
                    },
                    transparencyMode: uvs.textureInfo.transparencyMode() as u8,
                })).or_default().push((g.faceType, &f));
            } else {
                facesGroupedByTexture.entry(None).or_default().push((g.faceType, &f));
            }
        }
    }

    #[derive(Default, Copy, Clone, bytemuck::NoUninit)]
    #[repr(C)]
    struct WeaponVertex {
        position: Vector3,
        uv: Vector2,
        color: Vector3,
    }

    let mut primitives = vec![];

    for (texturePageKey, faces) in facesGroupedByTexture {
        let mut twoPagesWide = false;
        if let Some(texturePageKey) = &texturePageKey {
            if texturePageKey.whichTexture.mode == 1 {
                for (_faceType, face) in faces.iter() {
                    if let Some(uvs) = &face.uvs {
                        twoPagesWide = twoPagesWide || uvs.uv1.u > 127 || uvs.uv2.u > 127 || uvs.uv3.u > 127;
                        if let Some(uv4) = uvs.uv4 {
                            twoPagesWide = twoPagesWide || uv4.u > 127;
                        }
                    }
                }
            }
        }

        let texturePage = texturePageKey.as_ref().map(|mti| vram.get_texture_page(mti.whichTexture, twoPagesWide, doc));
        let occupiedRect = texturePage.map(|tp| tp.rect).unwrap_or(Rect { x: 0, y: 0, width: 128, height: 256 });
            
        let mut vertices: Vec<WeaponVertex> = vec![];
        let mut indices: Vec<u16> = vec![];

        #[derive(Hash, PartialEq, Eq, Clone, Copy)]
        struct VertexKey {
            originalIndex: u16,
            u: u8, v: u8,
            r: u8, g: u8, b: u8,
        }
        let mut keyToIndexMapping = std::collections::HashMap::<VertexKey, u16>::new();
        let mut getVertexIndex = |key: VertexKey| {
            if keyToIndexMapping.contains_key(&key) { return keyToIndexMapping[&key]; }
            let v = &geo.vertices[key.originalIndex as usize];
            keyToIndexMapping.insert(key, vertices.len() as u16);
            vertices.push(WeaponVertex {
                position: v.position(),
                uv: vec2(
                    (key.u as f32 - occupiedRect.x as f32) / occupiedRect.width as f32,
                    (key.v as f32 - occupiedRect.y as f32) / occupiedRect.height as f32
                ),
                color: vec3(key.r as f32 / 128.0, key.g as f32 / 128.0, key.b as f32 / 128.0),
            });
            vertices.len() as u16 - 1
        };

        for (faceType, f) in faces {
            let faceIndices = match &f.indices[..] { quad@&[_, _, _, _] => quad, &[a, b, c] => &[b, c, a][..], _ => unreachable!() }; // Shuffle triangle indices for mysterious reasons
            let uvs = if let Some(uvs) = &f.uvs {
                if faceType.isQuad() { &[uvs.uv1, uvs.uv2, uvs.uv3, uvs.uv4.unwrap()][..] } else { &[uvs.uv1, uvs.uv2, uvs.uv3][..] }
            } else {
                const Z: UV = UV { u: 0, v: 0 };
                if faceType.isQuad() { &[Z, Z, Z, Z][..] } else { &[Z, Z, Z][..] }
            };
            let mappedIndices: Vec<u16> = faceIndices.iter().zip(uvs).enumerate().map(|(vi, (&i, uv))| {
                let vertexColor = if faceType.vertexColorPerVertex() {
                    if faceType.isQuad() || faceType.isTextured() {
                        &f.vertexColors[vi]
                    } else {
                        &f.vertexColors[(vi + 1) % 3] // https://www.chronocompendium.com/Term/Mesh.html untextured triangle vertex colors are further shuffled
                    }
                } else {
                    &f.vertexColors[0]
                };
                getVertexIndex(VertexKey {
                    originalIndex: i,
                    u: uv.u, v: uv.v,
                    r: vertexColor.r, g: vertexColor.g, b: vertexColor.b,
                })
            }).collect();
            match &mappedIndices[..] {
                &[a, b, c, d] => indices.extend_from_slice(&[a, c, b, b, c, d]),
                &[a, b, c] => indices.extend_from_slice(&[a, c, b]),
                _ => unreachable!(),
            }
        }

        let mut minPosition = vertices[0].position;
        let mut maxPosition = vertices[0].position;
        for v in &vertices {
            minPosition = minPosition.min(v.position);
            maxPosition = maxPosition.max(v.position);
        }

        let vertexBytes: &[u8] = bytemuck::cast_slice(vertices.as_slice());
        let indicesBytes: &[u8] = bytemuck::cast_slice(indices.as_slice());

        // Add material
        // TODO: Blending will require more correct handling of the semi-transparency bit
        let (godot_blend_mode, baseColorFactor, alphaMode) = match texturePageKey.as_ref().map(|mti| mti.transparencyMode) {
            _ => ("mix", vec4(1.0, 1.0, 1.0, 1.0), "MASK"), // :[ mad because lots of meshes are flagged add when they're fully opaque, need to handle textures correctly I guess
            //None | Some(0) => ("mix", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
            // Some(0) => ("mix", vec4(1.0, 1.0, 1.0, 0.5), "BLEND"), // Since this is the default, probably best to find a solution to the semi-transparency bit first
            // Some(1) => ("add", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
            // Some(2) => ("subtract", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
            // Some(3) => ("add", vec4(0.25, 0.25, 0.25, 1.0), "MASK"),
            // _ => unreachable!(),
        };
        let material = doc.add("materials", object! {
            "name": name.to_string() + "Material",
            "pbrMetallicRoughness": {
                "metallicFactor": 0,
                "baseColorFactor": baseColorFactor.as_ref().as_slice(),
                // Insert baseColorTexture here if textured
            },
            "alphaMode": alphaMode,
            "extensions": { // These meshes have no normals, and godot dislikes that
                "KHR_materials_unlit": {}
            },
            "extras": { "blend_mode": godot_blend_mode },
        });
        if let Some(texturePage) = texturePage {
            // The tiling floor models for battlefields need nearest filtering to prevent showing seams
            let sampler = doc.add("samplers", object! {
                magFilter: NEAREST,
                minFilter: NEAREST,
                wrapS: CLAMP_TO_EDGE,
                wrapT: CLAMP_TO_EDGE,
            });
            let texture = doc.add("textures", object! {
                "source": texturePage.image,
                "sampler": sampler,
                "name": name.to_string() + "Texture",
            });
            doc.root["materials"][material]["pbrMetallicRoughness"]["baseColorTexture"] = object! { "index": texture };
        }

        // Add mesh
        let indexBuffer = doc.add("buffers", object! {
            "uri": bytesToURI(indicesBytes),
            "byteLength": indicesBytes.len(),
        });
        let indexBufferView = doc.add("bufferViews", object! {
            "buffer": indexBuffer,
            "byteLength": indicesBytes.len(),
            "target": ELEMENT_ARRAY_BUFFER,
        });
        let indexAccessor = doc.add("accessors", object! {
            "bufferView": indexBufferView, "byteOffset": 0, "componentType": UNSIGNED_SHORT, "count": indices.len(), "type": "SCALAR"
        });

        let vertexBuffer = doc.add("buffers", object! {
            "uri": bytesToURI(&vertexBytes),
            "byteLength": vertexBytes.len(),
        });
        let vertexBufferView = doc.add("bufferViews", object! {
            "buffer": vertexBuffer,
            "byteLength": vertexBytes.len(),
            "target": ARRAY_BUFFER,
            "byteStride": std::mem::size_of::<WeaponVertex>(),
        });
        let positionAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(WeaponVertex, position), "type": "VEC3", "componentType": FLOAT, "count": vertices.len(),
            "min": [minPosition.x, minPosition.y, minPosition.z],
            "max": [maxPosition.x, maxPosition.y, maxPosition.z]
        });
        let uvAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(WeaponVertex, uv), "type": "VEC2", "componentType": FLOAT, "count": vertices.len()
        });
        let vertexColorAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(WeaponVertex, color), "type": "VEC3", "componentType": FLOAT,  "count": vertices.len()
        });
        primitives.push(object! {
            "attributes": {
                "POSITION": positionAccessor,
                "TEXCOORD_0": uvAccessor,
                "COLOR_0": vertexColorAccessor,
            },
            "indices": indexAccessor,
            "mode": 4,
            "material": material
        });
    }

    doc.add("meshes", object! {
        "primitives": primitives,
        "name": format!("{}Mesh", name),
    })
}

fn prdNameToString(name: &[u8; 4]) -> String {
    let mut result = String::new();
    for &c in name {
        if c == 0 { break; }
        result.push(c as char);
    }
    result
}

fn convert_prd(
    filesSource: &mut FilesSource,
    filename: &str,
    outputType: OutputType,
) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let name = path_stem(filename);
    let fileBytes = filesSource.read_file(filename)?;
    let prd = prd::PRD::read(&mut Cursor::new(&fileBytes))?;

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };

    let mut tims = vec![];
    for file in prd.files.iter() {
        if let prd::FileType::TIM = file.file_type {
            tims.push(tim::TIM::read(&mut Cursor::new(&file.contents))?);
        }
    }

    let mut vram = Vram::new();
    for tim in tims.iter() {
        vram.add_tim(tim);
    }
    
    let mut texturePath = std::path::PathBuf::from(&filename);
    texturePath.set_extension("ltd");
    eprintln!("Checking for texture at {:?}", texturePath);
    if let Ok(ltdBin) = filesSource.read_file(&texturePath) {
        if let Ok(ltdTex) = ltd::LTD::read(&mut Cursor::new(&ltdBin)) {
            vram.add_ltd(&ltdTex);
        }
    }
    texturePath.set_extension("ctd");
    eprintln!("Checking for texture at {:?}", texturePath);
    if let Ok(ctdBin) = filesSource.read_file(&texturePath) {
        if let Ok(ctdFile) = chunkctd::ChunkCTD::read(&mut Cursor::new(&ctdBin)) {
            vram.add_ctd(&ctdFile);
        }
    }

    let mut meshes = std::collections::HashMap::<([u8; 4], u8), usize>::new();
    let mut usedMeshes = std::collections::HashSet::<([u8; 4], u8)>::new();

    const BMESH_TYPE: u8 = 0x13;
    const MESH_TYPE: u8 = 0x0e;
    // const TIM_TYPE: u8 = 0x03;
    // const MODEL_TYPE: u8 = 0x14; // saw this in tgolem.prd @ 0x1418
    // 0x15 in bg_00.prd 'lenz'


    let mut nodes = vec![];
    for file in prd.files.iter() {
        let name = prdNameToString(&file.name);
        if let prd::FileType::BATTLEFIELD_MESH = file.file_type {
            let geometry = weapbin::WeaponGeometry::read(&mut Cursor::new(&file.contents))?;
            let mesh = add_weapon_mesh(&mut doc, &name, &mut vram, &geometry);
            if meshes.contains_key(&(file.name, BMESH_TYPE)) {
                eprintln!("WARNING battlefield mesh already exists: {}", name);
            }
            meshes.insert((file.name, BMESH_TYPE), mesh);
        } else if let prd::FileType::MESH = file.file_type {
            let geometry = weapbin::WeaponGeometry::read(&mut Cursor::new(&file.contents))?;
            let mesh = add_weapon_mesh(&mut doc, &name, &mut vram, &geometry);
            if meshes.contains_key(&(file.name, MESH_TYPE)) {
                eprintln!("WARNING mesh already exists: {}", name);
            }
            meshes.insert((file.name, MESH_TYPE), mesh);
        } else if let prd::FileType::MDL = file.file_type {
            let model = readModel(&mut Cursor::new(&file.contents))?;
            let mut animations = vec![];
            for (animIndex, anim) in model.animationData.animations.iter().enumerate() {
                animations.push((format!("{}Anim{}", name, animIndex), build_animation_buffer(&model.transformData.joints, anim, animIndex)));
            }
            // TODO: where does a model store its texture x/y and clut x/y?
            nodes.push(add_model_to_output(&make_magenta_checkerboard(128, 256, 16), &model, outputType, &name, &animations, &mut doc)?)
        }
    }

    for file in prd.files.iter().filter(|file| matches!(file.file_type, prd::FileType::UNKNOWN2)) {
        use meshinstance::MeshCommand as mc;
        let meshInstance = meshinstance::MeshInstance::read(&mut Cursor::new(&file.contents))?;
        let mut meshName: Option<([u8; 4], u8)> = None;
        let mut translation = Vector3::ZERO;
        let mut rotation = Quaternion::IDENTITY;
        let mut scale = Vector3::ONE;
        let mut color = Vector3::ONE;
        let mut blend_override = "none".to_string();
        let mut animations = vec![];
        let mut unknown_commands = vec![];
        for command in meshInstance.commandList.commands.iter() {
            match &command.command {
                mc::End => {
                    // Do nothing
                },
                &mc::Mesh { name, refType, .. } => {
                    meshName = Some((name, refType));
                },
                &mc::BlendMode { blend_mode, .. } => {
                    blend_override = match blend_mode {
                        0 => "opaque".to_string(),
                        1 => "mix".to_string(),
                        2 => "add".to_string(),
                        3 => "subtract".to_string(),
                        4 => "quarter_add".to_string(),
                        _ => format!("unknown{}", blend_mode), // Seen 5, 6, 16, 18, 130 see e.g. battle/effects/tech/hardhit.prd
                    };
                },
                &mc::Translation { x, y, z } => {
                    translation = formats::cc_position(x, y, z);
                },
                &mc::Velocity { x, y, z } => {
                    let velocity = formats::cc_position(x, y, z);
                    animations.push(object! {
                        "type": "velocity",
                        "value": velocity.as_ref().as_slice(),
                    });
                },
                &mc::Acceleration { x, y, z } => {
                    let acceleration = formats::cc_position(x, y, z);
                    animations.push(object! {
                        "type": "acceleration",
                        "value": acceleration.as_ref().as_slice(),
                    });
                },
                &mc::Ttl { ttl } => {
                    if ttl != 0 {
                        animations.push(object! {
                            "type": "spawner",
                            "interval": meshInstance.spawnInterval,
                            "ttl": ttl,
                        });
                    }
                },
                &mc::Scale { x, y, z } => {
                    // Do this so godot doesn't complain about 0-length basis vectors
                    let x = if x == 0 { 1.0 / 32.0 } else { x as f32 };
                    let y = if y == 0 { 1.0 / 32.0 } else { y as f32 };
                    let z = if z == 0 { 1.0 / 32.0 } else { z as f32 };

                    scale = vec3(x as f32 / 4096.0, y as f32 / 4096.0, z as f32 / 4096.0);
                },
                mc::SinusoidalTranslation { animation: st } => {
                     let amplitude = formats::cc_position(st.x.amplitude, st.y.amplitude, st.z.amplitude);

                    animations.push(object! {
                        "type": "sinusoidal_translation",
                        "amplitude": amplitude.as_ref().as_slice(),
                        "speed": (vec![st.x.speed, st.y.speed, st.z.speed]),
                    });
                },
                &mc::ScaleVelocity { x, y, z } => {
                    let scaleVel = vec3(x as f32 / 4096.0, y as f32 / 4096.0, z as f32 / 4096.0);
                    animations.push(object! {
                        "type": "scale_velocity",
                        "value": scaleVel.as_ref().as_slice(),
                    });
                },
                &mc::Rotation { x, y, z } => {
                    rotation = formats::cc_mesh_quaternion(x, y, z);
                },
                &mc::RotationVelocity { x, y, z } => {
                    let angular_velocity = formats::euler_angles(x, y, z);
                    animations.push(object! { "type": "angular_velocity", "value":  angular_velocity.as_ref().as_slice() });
                },
                &mc::Color { r, g, b } => {
                    color = vec3(r as f32 / 2048.0, g as f32 / 2048.0, b as f32 / 2048.0);
                },
                mc::SinusoidalColor { animation: st } => {
                     let amplitude = vec3(st.x.amplitude as f32 / 2048.0, st.y.amplitude as f32 / 2048.0, st.z.amplitude as f32 / 2048.0);

                    animations.push(object! {
                        "type": "sinusoidal_color",
                        "amplitude": amplitude.as_ref().as_slice(),
                        "speed": (vec![st.x.speed, st.y.speed, st.z.speed]),
                    });
                },
                &mc::ColorVelocity { r, g, b } => {
                    let colorVel = vec3(r as f32 / 2048.0, g as f32 / 2048.0, b as f32 / 2048.0);

                    animations.push(object! {
                        "type": "color_velocity",
                        "value": colorVel.as_ref().as_slice(),
                    });
                },
                mc::Unknown { command: c, args } => {
                    let mut arr = jzon::array![ format!("0x{:x}", c) ];
                    for &arg in args.iter() {
                        arr.push(arg).unwrap();
                    }
                    unknown_commands.push(arr);
                }
            }
        }
        if let Some(meshName) = meshName {
            if let Some(&mesh) = meshes.get(&meshName) {
                usedMeshes.insert(meshName);
                nodes.push(doc.add("nodes", object! {
                    "name": format!("{}-{}", prdNameToString(&file.name), prdNameToString(&meshName.0)),
                    "mesh": mesh,
                    "translation": translation.as_ref().as_slice(),
                    "rotation": rotation.as_ref().as_slice(),
                    "scale": scale.as_ref().as_slice(),
                    "extras": {
                        "godot_tint": color.as_ref().as_slice(),
                        "godot_blend_override": blend_override,
                        "animations": animations,
                        "unknown_commands": unknown_commands,
                    }
                }));
            } else {
                // smok gets printed here by bg_17.prd, smok exists as a tim in battle/battle.prd, it also uses type 0x03 instead of 0x13 or 0x0e
                eprintln!("Unknown mesh {:?}, skipping", meshName);
            }
        } else {
            eprintln!("WARNING: Mesh instance without name??");
        }
    }

    // Give all unused meshes a node as well
    for (_, mesh) in meshes.into_iter().filter(|(name, _)| !usedMeshes.contains(name)) {
        let name = doc.root["meshes"][mesh]["name"].clone();
        nodes.push(doc.add("nodes", object! {
            "name": name,
            "mesh": mesh,
        }));
    }

    // Add camera paths
    for file in prd.files.iter().filter(|file| matches!(file.file_type, prd::FileType::CAMERA_PATH)) {
        let cam = camera_path::CameraPath::read(&mut Cursor::new(&file.contents))?;
        let mut camera_path = vec![];
        for entry in cam.entries.iter() {
            let p1 = formats::cc_position(entry.pos_x, entry.pos_y, entry.pos_z);
            let p2 = formats::cc_position(entry.focus_x, entry.focus_y, entry.focus_z);
            camera_path.push((p1, p2));
        }
        nodes.push(doc.add("nodes", object! {
            "name": format!("{}.CameraPath", prdNameToString(&file.name)),
            "extras": {
                "camera_anim_duration": cam.mystery1,
                "point_pairs": (camera_path.into_iter().map(|(p1, p2)|
                    object ! {
                        "position": p1.as_ref().as_slice(),
                        "focus": p2.as_ref().as_slice(),
                    }
                ).collect::<Vec<_>>())
            },
        }));
    }

    // Blender doesn't need a scene, but godot does
    let baseScene = doc.add("scenes", object! {
        "nodes": nodes
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn dump_prd(
    filesSource: &mut FilesSource,
    filename: &str,
    dump_path: &str,
) -> Result<()> {
    let fileBytes = filesSource.read_file(filename)?;
    let prd = prd::PRD::read(&mut Cursor::new(&fileBytes))?;
    let csv_base_path: &std::path::Path = dump_path.as_ref();
    let name = path_stem(filename);

    for file in prd.files.iter().filter(|file| matches!(file.file_type, prd::FileType::UNKNOWN2)) {
        let meshInstance = meshinstance::MeshInstance::read(&mut Cursor::new(&file.contents))?;
        let mut meshName: Option<[u8; 4]> = None;
        for command in meshInstance.commandList.commands.iter() {
            match &command.command {
                meshinstance::MeshCommand::Mesh { name, .. } => meshName = Some(name.clone()),
                _ => (),
            }
        }
        let meshName = if let Some(meshName) = meshName { prdNameToString(&meshName) } else { "unknown".to_string() };
        for command in meshInstance.commandList.commands.iter() {
            match &command.command {
                meshinstance::MeshCommand::Unknown { command, args } => {
                    use std::io::Write;
                    let csv_path = csv_base_path.join(format!("{:02x}.csv", command));
                    let mut to_append_to = std::fs::File::options().append(true).create(true).open(csv_path)?;
                    write!(&mut to_append_to, "{}:{}-{}", name, prdNameToString(&file.name), meshName)?;
                    for arg in args.iter() {
                        write!(&mut to_append_to, ", {:02x}", arg)?;
                    }
                    writeln!(&mut to_append_to, "")?;
                },
                _ => ()
            }
        }
    }

    Ok(())
}

fn convert_mesh(
    filesSource: &mut FilesSource,
    filename: &str,
    outputType: OutputType,
) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let name = path_stem(filename);
    let fileBytes = filesSource.read_file(filename)?;
    let mesh = weapbin::WeaponGeometry::read(&mut Cursor::new(&fileBytes))?;

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };

    let mesh = add_weapon_mesh(&mut doc, &name, &mut Vram::new(), &mesh);
    let node = doc.add("nodes", object! {
        "name": name.to_string() + "Mesh",
        "mesh": mesh,
    });

    // Blender doesn't need a scene, but godot does
    let baseScene = doc.add("scenes", object! {
        "nodes": [node]
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn convert_weapon_model(
    filesSource: &mut FilesSource,
    filename: &str,
    outputType: OutputType,
) -> Result<String> {
    let fileBytes = filesSource.read_file(filename)?;

    let w = weapbin::WeapBin::read(&mut Cursor::new(&fileBytes))?;

    let name = path_stem(filename);

    eprintln!("Num weapons {}", w.weapons.len());
    if matches!(outputType, OutputType::Obj) {
        use std::fmt::Write;
        let mut output = String::new();
        let mut b = 1; // obj indices start from 1
        let mut vti = 1;
        let mtllib_name = format!("{}-weapon-material.mtl", name);
        let mut mtllib = std::fs::File::create(&mtllib_name)?;
        println!("mtllib {}", mtllib_name);
        for (index, weapon) in w.weapons.iter().enumerate() {
            let texSize = (weapon.tim.pixels.width * 4, weapon.tim.pixels.height);
            let mtl_name = format!("{}{}mat", name, index);
            {
                use std::io::Write;
                let timpng_filename = format!("{}-{}-weapon-texture.png", name, index);
                writeln!(mtllib, "newmtl {}", mtl_name)?;
                writeln!(mtllib, "map_Kd {}", timpng_filename)?;
                eprintln!("Exporting tim to png: {}", timpng_filename);
                std::fs::write(timpng_filename, &weapon.tim.to_png())?;
            }
            dbg!((index, weapon.geo.vertices.len()));
            writeln!(output, "o {}{}", name, index)?;
            writeln!(output, "usemtl {}", mtl_name)?;
            for v in weapon.geo.vertices.iter() {
                let mut v = v.position();
                v.x += index as f32 * 1.0;
                writeln!(output, "v {} {} {}", v.x, v.y, v.z)?;
            }
            for g in &weapon.geo.groups {
                for f in g.faces.iter() {
                    let mut print_uv = |uv: UV| writeln!(output, "vt {} {}", uv.u as f32 / texSize.0 as f32, 1.0 - uv.v as f32 / texSize.1 as f32);
                    if let Some(uvs) = &f.uvs {
                        print_uv(uvs.uv1)?;
                        print_uv(uvs.uv2)?;
                        print_uv(uvs.uv3)?;
                        if g.faceType.isQuad() {
                            print_uv(uvs.uv4.unwrap())?;
                        }
                    } else {
                        // TODO: this is wrong, it needs to not sample a texture at all
                        for _ in 0..g.faceType.num_verts() {
                            print_uv(UV { u: 0, v: 0 })?;
                        }
                    }
                }
            }
            for g in &weapon.geo.groups {
                for f in g.faces.iter() {
                    if g.faceType.isQuad() {
                        writeln!(output, "f {}/{} {}/{} {}/{} {}/{}",
                            f.indices[0] + b, vti + 0,
                            f.indices[2] + b, vti + 2,
                            f.indices[3] + b, vti + 3,
                            f.indices[1] + b, vti + 1
                        )?; // unkrangle the quads
                    } else {
                        writeln!(output, "f {}/{} {}/{} {}/{}",
                            f.indices[0] + b, vti + 0,
                            f.indices[2] + b, vti + 2,
                            f.indices[1] + b, vti + 1
                        )?; // unwind the tris
                    }
                    vti += g.faceType.num_verts();
                }
            }
            b += weapon.geo.vertices.len() as u16;
        }
        return Ok(output);
    }

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };

    let mut nodes: Vec<usize> = vec![];

    for (index, weapon) in w.weapons.iter().enumerate() {
        let name = format!("{}{}", name, index);
        let mut vram = Vram::new();
        vram.add_tim(&weapon.tim);
        let mesh = add_weapon_mesh(&mut doc, &name, &mut vram, &weapon.geo);
        nodes.push(doc.add("nodes", object! {
            "name": name.to_string() + "Mesh",
            "mesh": mesh,
        }));
    }

    // Blender doesn't need a scene, but godot does
    let baseScene = doc.add("scenes", object! {
        "nodes": nodes
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn read_bin_animations(filesSource: &mut FilesSource, bin_path: &std::path::Path, model: &Model) -> Result<Vec<AnimationBuffer>> {
    eprintln!("Attempting to read {:?}", bin_path);
    let binContents = filesSource.read_file(&bin_path)?;
    eprintln!("Attempting to parse {:?}", bin_path);
    let binBundle = cpt::CPT::read(&mut Cursor::new(&binContents))?;
    eprintln!("Reading animations...");
    // https://www.chronocompendium.com/Term/Chrono_Cross_Player_Character_Overworld_Animation_Files.html
    // Read the animations from the first section
    let firstSectionContents = &binBundle.entries.get(0).ok_or_else(|| anyhow::anyhow!("animation bin lacked section 0"))?.contents;
    let anims = AnimationData::read_args(&mut Cursor::new(&firstSectionContents), (model.transformData.joints.len() as u32,))?;
    eprintln!("Read {} animations", anims.animations.len());
    let mut animations = vec![];
    for (animIndex, anim) in anims.animations.iter().enumerate() {
        animations.push(build_animation_buffer(&model.transformData.joints, anim, animIndex));
    }
    Ok(animations)
}

fn read_prd_animations(filesSource: &mut FilesSource, prd_path: &std::path::Path, prd_entry_name: &str, model: &Model) -> Result<Vec<AnimationBuffer>> {
    eprintln!("Attempting to read {:?}", prd_path);
    let prdContents = filesSource.read_file(&prd_path)?;
    eprintln!("Attempting to parse {:?}", prd_path);
    let prdBundle = prd::PRD::read(&mut Cursor::new(&prdContents))?;
    prdBundle.sanity_check()?;
    eprintln!("Looking for {} file in {:?}", prd_entry_name, prd_path);
    let prdFile = prdBundle.first_file_matching_name(prd_entry_name).ok_or(anyhow::anyhow!("prd file had no {} subfile", prd_entry_name))?;
    eprintln!("Found {} file", prd_entry_name);
    if !matches!(prdFile.file_type, prd::FileType::ANIM) { return Err(anyhow::anyhow!("prd file had {} with wrong file type {:?}", prd_entry_name, prdFile.file_type)); }
    eprintln!("Reading animations...");
    let anims = AnimationData::read_args(&mut Cursor::new(&prdFile.contents), (model.transformData.joints.len() as u32,))?;
    eprintln!("Read {} animations", anims.animations.len());
    let mut animations = vec![];
    for (animIndex, anim) in anims.animations.iter().enumerate() {
        animations.push(build_animation_buffer(&model.transformData.joints, anim, animIndex));
    }
    Ok(animations)
}

fn read_attack_animations(filesSource: &mut FilesSource, model_path: &std::path::Path, model: &Model) -> Result<Vec<(String, AnimationBuffer)>> {
    let mut path = std::path::PathBuf::from(model_path);
    path.set_file_name("at0.prd");
    let animations = read_prd_animations(filesSource, path.as_ref(), "at0\0", model)?;
    Ok(animations.into_iter().enumerate().map(|(animIndex, animation)| (format!("Attack{}", animIndex), animation)).collect())
}

struct OutputVertex {
    pos: Vector3,
    normal: Vector3,
    joints: [u8; 2],
    weights: [i16; 2],
}

#[derive(Copy, Clone, bytemuck::NoUninit)]
#[repr(C)]
struct FatVertex {
    pos: Vector3,
    normal: Vector3,
    uv: Vector2,
    color: Vector3,
    joints: [u8; 4],
    weights: Vector4,
}

struct GeneratedMesh {
    vertices: Vec<FatVertex>,
    faces: Vec<Vec<u16>>,
    num_flipped_normals: usize,
}

fn create_facegroup_mesh(faceGroup: &FaceGroup, outputVertices: &[OutputVertex], textureInfo: &TextureInfo, triangulate: bool, flipDotThreshold: f32) -> GeneratedMesh {    
    let numFlippedNormals = std::cell::Cell::new(0);

    #[derive(Hash, PartialEq, Eq, Clone, Copy)]
    struct VertexKey {
        originalIndex: u16,
        u: u16, v: u16,
        r: u8, g: u8, b: u8,
        flipNormal: bool,
    }

    let mut fatVerts: Vec<FatVertex> = vec![];
    let mut faceVerts = std::collections::HashMap::<VertexKey, u16>::new();
    let mut getVertex = |key: VertexKey, forceNew: bool| {
        if key.flipNormal {
            numFlippedNormals.set(numFlippedNormals.get() + 1);
        }
        if !forceNew {
            if let Some(index) = faceVerts.get(&key) { return *index; }
        }
        let index = fatVerts.len();
        faceVerts.insert(key, index as u16);
        let uv = vec2((key.u as f32 + 0.5) / textureInfo.size.0 as f32, (key.v as f32 + 0.5) / textureInfo.size.1 as f32);
        let ov = &outputVertices[key.originalIndex as usize];
        let weightTotal = (ov.weights[0] + ov.weights[1]) as f32;
        fatVerts.push(FatVertex {
            pos: ov.pos,
            normal: ov.normal * if key.flipNormal { -1.0 } else { 1.0 },
            uv,
            color: vec3(key.r as f32 / 255.0, key.g as f32 / 255.0, key.b as f32 / 255.0),
            joints: [ov.joints[0], ov.joints[1], 0, 0],
            weights: vec4(ov.weights[0] as f32 / weightTotal, ov.weights[1] as f32 / weightTotal, 0.0, 0.0),
        });
        index as u16
    };

    let mut existingFaces = std::collections::HashSet::<Vec<u16>>::new();
    let mut outputFatFaces: Vec<Vec<u16>> = vec![];

    fn sorted(v: &Vec<u16>) -> Vec<u16> {
        let mut v = v.clone();
        v.sort();
        v
    }

    // Faces

    let mut addFace = |faceType: &FaceType, face: &Face, faceIndices: &[u16], order: &[u8]| {
        let i1 = faceIndices[order[0] as usize] as usize;
        let i2 = faceIndices[order[1] as usize] as usize;
        let i3 = faceIndices[order[2] as usize] as usize;
        let v1 = outputVertices[i1].pos;
        let v2 = outputVertices[i2].pos;
        let v3 = outputVertices[i3].pos;
        let faceNormal = (v2 - v1).cross(v3 - v1).normalize();

        let mut faceVert = |fvi: u8, force: bool| {
            let uv: &UV = &face.textureUV[fvi as usize];
            let second_texture_page = faceType.isTextured() && face.indices[0] & 0b1111 == 1 && face.indices[1] & 0b1111 == 1;
            let u = if second_texture_page { uv.u as u16 + 128 } else { uv.u as u16 };
            let v = uv.v as u16;
            let color = &face.vertexColor;
            let n = outputVertices[faceIndices[fvi as usize] as usize].normal;
            getVertex(VertexKey {
                originalIndex: faceIndices[fvi as usize],
                u, v,
                r: color.r, g: color.g, b: color.b,
                flipNormal: n.dot(faceNormal) < flipDotThreshold,
            }, force)
        };

        let mut outputFace: Vec<u16> = order.iter().map(|&pi| faceVert(pi, false)).collect();

        // Check if this poly already exists (probably with opposite winding) because blender doesn't like that
        let mut selfSorted = sorted(&outputFace);
        if existingFaces.contains(&selfSorted) {
            // Force a new unique vertex
            outputFace[0] = faceVert(order[0], true);
            selfSorted = sorted(&outputFace);
        }
        existingFaces.insert(selfSorted);
        outputFatFaces.push(outputFace);
    };

    let (indices, _extraData) = calcIndices(faceGroup);
    let indices = &mut indices.into_iter();
    for chunk in faceGroup.chunks.iter() {
        for face in chunk.faces.iter() {
            let num_verts = chunk.faceType.num_verts();
            let faceIndices: Vec<u16> = indices.take(num_verts).collect();
            assert_eq!(faceIndices.len(), num_verts);
            assert!(face.textureUV.len() >= num_verts);
            match &faceIndices[..] {
                &[_, _, _] => addFace(&chunk.faceType, &face, &faceIndices, &[0, 2, 1][..]), // Change winding order
                &[_, _, _, _] => {
                    if triangulate {
                        // https://psx-spx.consoledev.net/graphicsprocessingunitgpu/#gpu-render-polygon-commands
                        // According to this source, PSX always splits quads as {0, 1, 2}, {1, 2, 3}
                        addFace(&chunk.faceType, &face, &faceIndices, &[0, 2, 1][..]); // First tri with changed winding order
                        addFace(&chunk.faceType, &face, &faceIndices, &[1, 2, 3][..]); // Second tri with changed winding order
                    } else {
                        addFace(&chunk.faceType, &face, &faceIndices, &[0, 2, 3, 1][..]); // Change winding order, and untwist quad for blender
                    }
                },
                _ => panic!("Polygon with number of verts != 3 or 4"),
            };
        }
    }

    GeneratedMesh {
        vertices: fatVerts,
        faces: outputFatFaces,
        num_flipped_normals: numFlippedNormals.get(),
    }
}

fn add_model_to_output(textureInfo: &TextureInfo, model: &Model, outputType: OutputType, name: &str, animations: &[(String, AnimationBuffer)], doc: &mut gltf::Gltf) -> Result<usize> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    eprintln!("Texture size: {:?}", textureInfo.size);

    let mut mats = vec![];
    for joint in model.transformData.joints.iter() {
        let mut mat = joint.transform.matrix();
        if  joint.parent != -1 {
            mat = mats[ joint.parent as usize] * mat;
        }
        mats.push(mat);
    }

    let mut outputVertices = vec![];

    let mut i = 0;
    for fg in model.faceData.faceGroups.iter() {
        let mut oneJointVerts = fg.oneJointVertices.iter();
        let mut normals = fg.normals.iter();
        for vg in fg.oneJointVertexGroups.iter() {
            let mat = &mats[vg.joint as usize];
            for _ in 0..vg.numVertices {
                let vert = oneJointVerts.next().unwrap();

                let pos = applyTransform(vert, &mat);
                let normal = applyTransformNormal(normals.next().unwrap(), &mat);

                outputVertices.push(OutputVertex {
                    pos,
                    normal,
                    joints: [vg.joint as u8, 0],
                    weights: [1, 0],
                });

                assert_eq!(vert.index, i);
                i += 1;
            }
        }
        let mut twoJointVerts = fg.twoJointVertices.iter();
        for vg in fg.twoJointVertexGroups.iter() {
            let matA = &mats[vg.jointA as usize];
            let matB = &mats[vg.jointB as usize];
            let weightSum = vg.weightA as f32 + vg.weightB as f32;
            let weightA = vg.weightA as f32 / weightSum;
            let weightB = vg.weightB as f32 / weightSum;
            for _ in 0..vg.numVertices {
                let vertA = twoJointVerts.next().unwrap();
                let vertB = twoJointVerts.next().unwrap();
                let vA = applyTransform(vertA, &matA);
                let vB = applyTransform(vertB, &matB);
                let vFinal = vA * weightA + vB * weightB;

                let normalA = normals.next().unwrap();
                let normalB = normals.next().unwrap();
                let nA = applyTransformNormal(normalA, &matA);
                let nB = applyTransformNormal(normalB, &matB);
                let nFinal = nA * weightA + nB * weightB;

                outputVertices.push(OutputVertex {
                    pos: vFinal,
                    normal: nFinal,
                    joints: [vg.jointA as u8, vg.jointB as u8],
                    weights: [vg.weightA, vg.weightB],
                });

                assert_eq!(vertA.index, i);
                assert_eq!(vertB.index, i);
                i += 1;
            }
        }
    }

    let triangulate = true;
    let mut numFlippedNormals = 0;

    let mut meshes = vec![];
    for (fg, colorInfo) in model.faceData.faceGroups.iter().zip(&model.section3.colorInfos) {
        let flipDotThreshold = if colorInfo.flags.doubleSided() { 0.0 } else { -2.0 /* Aka disabled */ };
        let mesh = create_facegroup_mesh(fg, &outputVertices, &textureInfo, triangulate, flipDotThreshold);
        numFlippedNormals += mesh.num_flipped_normals;
        meshes.push(mesh);
    }


    eprintln!("Flipped {} normals", numFlippedNormals);

    // if matches!(outputType, OutputType::Obj) {
    //     let mut baseIndexOffset: usize = 0;
    //     for (meshIndex, mesh) in meshes.iter().enumerate() {
    //         let fatVerts = &mesh.vertices;
    //         let outputFatFaces = &mesh.faces;
    //         println!("o {}{}", name, meshIndex);
    //         // Output .obj
    //         for v in fatVerts {
    //             let v = v.pos;
    //             println!("v {} {} {}", v.x, v.y, v.z);
    //         }
    //         for v in fatVerts {
    //             let v = v.normal;
    //             println!("vn {} {} {}", v.x, v.y, v.z);
    //         }
    //         for v in fatVerts {
    //             println!("vt {} {}", v.uv.x, 1.0 - v.uv.y);
    //         }
    //         for face in outputFatFaces {
    //             print!("f ");
    //             for &index in face {
    //                 let index = baseIndexOffset + index as usize + 1;
    //                 print!(" {}/{}/{}", index, index, index);  // .obj is 1-based
    //             }
    //             print!("\n");
    //         }
    //         baseIndexOffset += fatVerts.len();
    //     }
    //     return Ok(usize::MAX);
    // }

    // let doc = doc.unwrap();

    let positionByteOffset = std::mem::offset_of!(FatVertex, pos);
    let normalByteOffset = std::mem::offset_of!(FatVertex, normal);
    let uvByteOffset = std::mem::offset_of!(FatVertex, uv);
    let colorByteOffset = std::mem::offset_of!(FatVertex, color);
    let jointsByteOffset = std::mem::offset_of!(FatVertex, joints);
    let weightsByteoffset = std::mem::offset_of!(FatVertex, weights);
    let vertexByteStride = std::mem::size_of::<FatVertex>();

    let inverseBindMatrices: Vec<_> = mats.iter().map(Matrix4::inverse).collect();
    let inverseBindMatricesBytes: &[u8] = bytemuck::cast_slice(&inverseBindMatrices[..]);

    let mut animationBufferBytes: Vec<u8> = vec![];

    let orientation = Matrix4::IDENTITY;
        //Matrix4::from_cols(vec4(1.0, 0.0, 0.0, 0.0), vec4(0.0, -1.0, 0.0, 0.0), vec4(0.0, 0.0, -1.0, 0.0), vec4(0.0, 0.0, 0.0, 1.0));

    // Add skeleton nodes
    let skeletonNode = doc.add("nodes", jzon::object! {
        "name": name.to_string() + "Model", 
        "matrix": orientation.as_ref().as_slice(),
    });
    let mut jointToNodeMapping = vec![];
    for (jointIndex, joint) in model.transformData.joints.iter().enumerate() {
        let jointNode = doc.add("nodes", object! {
            "name": format!("Joint{}", jointIndex),
            "translation": joint.transform.translation().as_ref().as_slice(),
            "rotation": joint.transform.quaternion().as_ref().as_slice(),
        });
        jointToNodeMapping.push(jointNode);
        if joint.parent > -1 {
            let parentObj = &mut doc.root["nodes"][jointToNodeMapping[joint.parent as usize]];
            gltf::pushToField(parentObj, "children", jointNode.into());
        } else {
            gltf::pushToField(&mut doc.root["nodes"][skeletonNode], "children", jointNode.into());
        }
    }

    // Add skin
    let ibmBuffer = doc.add("buffers", object! {
        "uri": bytesToURI(inverseBindMatricesBytes),
        "byteLength": inverseBindMatricesBytes.len(),
    });
    let ibmBufferView = doc.add("bufferViews", object! {
        "buffer": ibmBuffer,
        "byteLength": inverseBindMatricesBytes.len(),
    });
    let ibmAccessor = doc.add("accessors", object! {
        "bufferView": ibmBufferView, "type": "MAT4", "componentType": FLOAT, "count": inverseBindMatrices.len()
    });
    let skin = doc.add("skins", object! {
        "joints": jointToNodeMapping.as_slice(),
        "skeleton": skeletonNode,
        "inverseBindMatrices": ibmAccessor,
    });

    // Add material
    let image = doc.add("images", object! {
        "uri": textureInfo.uri(),
        "name": name.to_string() + "Image",
    });
    let texture = doc.add("textures", object! {
        "source": image,
        "name": name.to_string() + "Texture",
    });

    // Add mesh
    let mut gltfFaceGroupPrimitives = vec![];
    for (meshIndex, (mesh, colorInfo)) in meshes.iter().zip(&model.section3.colorInfos).enumerate() {
        let fatVerts = &mesh.vertices;
        let outputFatFaces = &mesh.faces;
        let indices: Vec<u16> = outputFatFaces.iter().flatten().cloned().collect();
        let indicesBytes: &[u8] = bytemuck::cast_slice(&indices[..]);

        let mut minPosition = fatVerts[0].pos;
        let mut maxPosition = fatVerts[0].pos;
        for v in fatVerts {
            minPosition = minPosition.min(v.pos);
            maxPosition = maxPosition.max(v.pos);
        }
        let dataBuffer: &[u8] = bytemuck::cast_slice(&fatVerts[..]);

        let indexBuffer = doc.add("buffers", object! {
            "uri": bytesToURI(indicesBytes),
            "byteLength": indicesBytes.len(),
        });
        let indexBufferView = doc.add("bufferViews", object! {
            "buffer": indexBuffer,
            "byteLength": indicesBytes.len(),
            "target": ELEMENT_ARRAY_BUFFER,
        });
        let indexAccessor = doc.add("accessors", object! {
            "bufferView": indexBufferView, "byteOffset": 0, "componentType": UNSIGNED_SHORT, "count": indices.len(), "type": "SCALAR"
        });

        let vertexBuffer = doc.add("buffers", object! {
            "uri": bytesToURI(&dataBuffer),
            "byteLength": dataBuffer.len(),
        });
        let vertexBufferView = doc.add("bufferViews", object! {
            "buffer": vertexBuffer,
            "byteLength": dataBuffer.len(),
            "target": ARRAY_BUFFER,
            "byteStride": vertexByteStride,
        });
        let positionAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": positionByteOffset, "type": "VEC3", "componentType": FLOAT, "count": fatVerts.len(),
            "min": [minPosition.x, minPosition.y, minPosition.z],
            "max": [maxPosition.x, maxPosition.y, maxPosition.z] 
        });
        let normalAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": normalByteOffset, "type": "VEC3", "componentType": FLOAT,  "count": fatVerts.len()
        });
        let uvAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": uvByteOffset, "type": "VEC2", "componentType": FLOAT, "count": fatVerts.len()
        });
        let vertexColorAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": colorByteOffset, "type": "VEC3", "componentType": FLOAT,  "count": fatVerts.len()
        });
        let jointAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": jointsByteOffset, "type": "VEC4", "componentType": UNSIGNED_BYTE,  "count": fatVerts.len()
        });
        let weightAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": weightsByteoffset, "type": "VEC4", "componentType": FLOAT,  "count": fatVerts.len()
        });

        // glTF doesn't support blend modes beyond mix, put that info into the material's 'extras' to be applied by godot
        // https://github.com/KhronosGroup/glTF/issues/1189
        // https://github.com/KhronosGroup/glTF/pull/1302

        let (godot_blend_mode, baseColorFactor, alphaMode) = match (colorInfo.flags.isSemiTransparent(), colorInfo.flags.semiTransparencyMode()) {
            (false, _) => ("mix", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
            (true, SEMI_TRANSPARENCY_MODE_MIX) => ("mix", vec4(1.0, 1.0, 1.0, 0.5), "BLEND"),
            (true, SEMI_TRANSPARENCY_MODE_ADD) => ("add", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
            (true, SEMI_TRANSPARENCY_MODE_SUBTRACT) => ("subtract", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
            (true, SEMI_TRANSPARENCY_MODE_ADD_ONE_FOURTH) => ("add", vec4(0.25, 0.25, 0.25, 1.0), "MASK"),
            _ => unreachable!(),
        };
        let material = doc.add("materials", object! {
            "name": format!("{}Material{}", name.to_string(), meshIndex),
            "pbrMetallicRoughness": {
                "metallicFactor": 0,
                "baseColorFactor": baseColorFactor.as_ref().as_slice(),
                "baseColorTexture": {
                    "index": texture,
                }
            },
            "alphaMode": alphaMode,
            "extras": { "blend_mode": godot_blend_mode },
        });

        gltfFaceGroupPrimitives.push(object! {
            "attributes": {
                "POSITION": positionAccessor,
                "NORMAL": normalAccessor,
                "TEXCOORD_0": uvAccessor,
                "COLOR_0": vertexColorAccessor,
                "JOINTS_0": jointAccessor,
                "WEIGHTS_0": weightAccessor,
            },
            "indices": indexAccessor,
            "mode": 4,
            "material": material
        });
    }

    let mesh = doc.add("meshes", object! {
        "primitives": gltfFaceGroupPrimitives,
        "name": name.to_string() + "Mesh"
    });
    let meshNode = doc.add("nodes", object! {
        "name": format!("{}Mesh", name),
        "mesh": mesh,
        "skin": skin,
    });

    // Add animations
    let animationBuffer = doc.add("buffers", object! {
        // Fill in uri and byteLength later
    });
    let animationBufferView = doc.add("bufferViews", object! {
        "buffer": animationBuffer
        // Fill in byteLength later
    });
    for (animName, anim) in animations.iter() {
        let mut animationObj = object! {
            "name": animName.as_str(),
            "samplers": [],
            "channels": [],
        };

        let timeBytes = bytemuck::cast_slice(anim.times.as_slice());
        let keyframeTimeAccessor = doc.add("accessors", object! {
            "bufferView": animationBufferView,
            "byteOffset": animationBufferBytes.len(),
            "type": "SCALAR", "componentType": FLOAT,
            "min": [anim.times[0]], "max": [anim.times[anim.times.len() - 1]],
            "count": anim.numFrames
        });
        let keyframeTimeSingleInitialFrameAccessor = doc.add("accessors", object! {
            "bufferView": animationBufferView,
            "byteOffset": animationBufferBytes.len(),
            "type": "SCALAR", "componentType": FLOAT,
            "min": [anim.times[0]], "max": [anim.times[0]],
            "count": 1
        });
        animationBufferBytes.extend(timeBytes);

        for channel in &anim.channels {
            let (outputType, animProperty, bytes, len) = match &channel.propValues {
                AnimatedJointType::Rotation(v) => {
                     ("VEC4", "rotation", bytemuck::cast_slice(&v), v.len())
                },
                AnimatedJointType::Translation(v) => {
                    ("VEC3", "translation", bytemuck::cast_slice(&v), v.len())
                }
            };
            if len > 1 { assert_eq!(len, anim.numFrames); }
            let outputAccessor = doc.add("accessors", object! {
                "bufferView": animationBufferView,
                "byteOffset": animationBufferBytes.len(),
                "type": outputType, "componentType": FLOAT,
                "count": len
            });
            animationBufferBytes.extend(bytes);
            
            let sampler = gltf::pushToField(&mut animationObj, "samplers", object! {
                "input": if len > 1 { keyframeTimeAccessor } else { keyframeTimeSingleInitialFrameAccessor },
                "interpolation": "LINEAR",
                "output": outputAccessor,
            });
             gltf::pushToField(&mut animationObj, "channels", object! {
                "sampler": sampler,
                "target": {
                    "node": jointToNodeMapping[channel.joint],
                    "path": animProperty,
                },
            });
        }

        doc.add("animations", animationObj);
    }
    // Fill in the uri and byteLength
    doc.root["buffers"][animationBuffer]["uri"] = bytesToURI(&animationBufferBytes).into();
    doc.root["buffers"][animationBuffer]["byteLength"] = animationBufferBytes.len().into();
    doc.root["bufferViews"][animationBufferView]["byteLength"] = animationBufferBytes.len().into();

    // Add attachment point nodes
    let mut attachmentPointNodes = vec![];
    for (i, point) in model.section3.attachmentPoints.iter().enumerate() {
        let attachmentPointNode = doc.add("nodes", object! {
            name: format!("AttachmentPoint{}", i),
            translation: point.position().as_ref().as_slice(),
        });
        attachmentPointNodes.push(attachmentPointNode);
        let parentNode = if point.joint > -1 && point.joint < jointToNodeMapping.len() as i16 {
            jointToNodeMapping[point.joint as usize]
        } else {
            eprintln!("WARNING, {}'s joint parent {} out of bounds: {}", name, i, point.joint);
            skeletonNode
        };
        gltf::pushToField(&mut doc.root["nodes"][parentNode], "children", attachmentPointNode.into());
    }

    // Add weapon attachment nodes
    for (i, wt) in model.section3.weaponTransforms.iter().enumerate() {
        let weaponAttachNode = doc.add("nodes", object! {
            name: format!("WeaponAttach{}", i),
            translation: wt.transform.translation().as_ref().as_slice(),
            rotation: wt.transform.quaternion().as_ref().as_slice(),
        });
        let parentNode = if wt.attachmentPoint > -1 && wt.attachmentPoint < attachmentPointNodes.len() as i16 {
            attachmentPointNodes[wt.attachmentPoint as usize]
        } else {
            eprintln!("WARNING, {}'s weapon transform {} attachment point out of bounds: {}", name, i, wt.attachmentPoint);
            skeletonNode
        };
        gltf::pushToField(&mut doc.root["nodes"][parentNode], "children", weaponAttachNode.into());
    }

    // Finally add the root scene
    gltf::pushToField(&mut doc.root["nodes"][skeletonNode], "children", meshNode.into());
    Ok(skeletonNode)
}

fn z_unlit_quad_with_texture(doc: &mut gltf::Gltf, texInfo: &TextureInfo, posA: Vector3, posB: Vector3, sampler: Option<usize>, name: &str) -> usize {
    let image = doc.add("images", object! {
        "uri": texInfo.uri(),
        "name": format!("{}Image", name),
    });
    let texture = doc.add("textures", object! {
        "source": image,
        "name": format!("{}Texture", name),
    });
    if let Some(sampler) = sampler {
        doc.root["textures"][texture]["sampler"] = sampler.into();
    }
    let material = doc.add("materials", object! {
        "name": format!("{}Material", name),
        "pbrMetallicRoughness": {
            "metallicFactor": 0,
            "baseColorTexture": {
                "index": texture,
            }
        },
        "alphaMode": "MASK",
        "extensions": {
            "KHR_materials_unlit": {}
        }
    });
    #[derive(Copy, Clone, bytemuck::NoUninit)]
    #[repr(C)]
    struct QuadVertex {
        position: Vector3,
        uv: Vector2,
    }
    let sx = posA.x;
    let ex = posB.x;
    let sy = posA.y;
    let ey = posB.y;
    let z = posA.z;
    let vertices = vec![
        QuadVertex { position: vec3(sx, sy, z), uv: vec2(0.0, 1.0) },
        QuadVertex { position: vec3(ex, sy, z), uv: vec2(1.0, 1.0) },
        QuadVertex { position: vec3(sx, ey, z), uv: vec2(0.0, 0.0) },

        QuadVertex { position: vec3(ex, sy, z), uv: vec2(1.0, 1.0) },
        QuadVertex { position: vec3(ex, ey, z), uv: vec2(1.0, 0.0) },
        QuadVertex { position: vec3(sx, ey, z), uv: vec2(0.0, 0.0) },
    ];

    let mut minPosition = vertices[0].position;
    let mut maxPosition = vertices[0].position;
    for v in &vertices {
        minPosition = minPosition.min(v.position);
        maxPosition = maxPosition.max(v.position);
    }

    let vertexBytes: &[u8] = bytemuck::cast_slice(&vertices);
    let vertexBuffer = doc.add("buffers", object! {
        "uri": bytesToURI(&vertexBytes),
        "byteLength": vertexBytes.len(),
    });
    let vertexBufferView = doc.add("bufferViews", object! {
        "buffer": vertexBuffer,
        "byteLength": vertexBytes.len(),
        "target": ARRAY_BUFFER,
        "byteStride": std::mem::size_of::<QuadVertex>(),
    });
    let positionAccessor = doc.add("accessors", object! {
        "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(QuadVertex, position), "type": "VEC3", "componentType": FLOAT, "count": vertices.len(),
        "min": [minPosition.x, minPosition.y, minPosition.z],
        "max": [maxPosition.x, maxPosition.y, maxPosition.z]
    });
    let uvAccessor = doc.add("accessors", object! {
        "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(QuadVertex, uv), "type": "VEC2", "componentType": FLOAT, "count": vertices.len()
    });
    let mesh = doc.add("meshes", object! {
        "primitives": [
            {
                "attributes": {
                    "POSITION": positionAccessor,
                    "TEXCOORD_0": uvAccessor,
                },
                "mode": 4,
                "material": material
            }
        ],
        "name": format!("{}Mesh", name),
    });
    doc.add("nodes", object! {
        "name": format!("{}", name),
        "mesh": mesh,
    })
}

fn room_model_texture(model: &Model, header: &mapbin::MapBinModelHeader, textures: &mapctd::MapTextureData) -> Option<ltd::RGBAImage> {
    // Some models embedded in map/mapbin/*.bin files implicitly use two adjacent textures from the corresponding *.ctd file, detect that case and create a combined 256x256 texture.
    // This assumes the texture uses 8-bit indices, with effective 128x256 pixel texture pages, and that the second tim file is at (+64, +0) (vram-space) offset from the first
    let textureLeft = textures.tims.iter().flat_map(|t| match &t.clut {
        Some(clut) if
            clut.x == header.textureClutX &&
            clut.y == header.textureClutY &&
            t.pixels.x == header.textureBaseX &&
            t.pixels.y == header.textureBaseY => Some(t),
        _ => None
    }).map(|t| t.to_rgba()).next();

    let uses_two_pages = model
        .faceData.faceGroups.iter()
        .flat_map(|f| f.chunks.iter())
        .flat_map(|c| c.faces.iter())
        .any(|face| face.indices[0] & 0b1111 == 1 && face.indices[1] & 0b1111 == 1);

    if uses_two_pages {
        let textureRight = if uses_two_pages {
            textures.tims.iter().flat_map(|t| match &t.clut {
                Some(_clut) if
                    t.pixels.x == header.textureBaseX + 64 &&
                    t.pixels.y == header.textureBaseY => Some(t),
                _ => None
            }).map(|t| t.to_rgba()).next()
        } else {
            None
        };
        let mut full_image = ltd::RGBAImage {
            width: 256,
            height: 256,
            pixels: vec![0; 256 * 256 * 4],
        };
        if textureLeft.is_none() && textureRight.is_none() {
            return None; // If neither texture was found, return None so that the caller can know there was an issue
        }
        if let Some(left) = textureLeft {
            full_image.blit(&left, 0, 0);
        } else {
            // If one or the other texture is missing, fill it in with a magenta checkerboard to make it obvious something went wrong
            full_image.blit(&make_magenta_checkerboard_image(128, 256, 16), 0, 0);
        }
        if let Some(right) = textureRight {
            full_image.blit(&right, 128, 0);
        } else {
            full_image.blit(&make_magenta_checkerboard_image(128, 256, 16), 128, 0);
        }
        Some(full_image)
    } else {
        textureLeft
    }
}

fn z_unlit_quad_with_texture_autosized(doc: &mut gltf::Gltf, textureInfo: &TextureInfo, sampler: Option<usize>) -> usize {
        let quadWidth = textureInfo.size.0 as f32 / 16.0;
        let quadHeight = textureInfo.size.1 as f32 / 16.0;
        let posA = vec3(0.0, 0.0, 0.0);
        let posB = vec3(quadWidth, quadHeight, 0.0);

        z_unlit_quad_with_texture(doc, &textureInfo, posA, posB, sampler, "Quad")
}

fn convert_effect_ctd(filesSource: &mut FilesSource, filename: &str, outputType: OutputType) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let fileBytes = filesSource.read_file(&filename)?;
    let ctdFile = chunkctd::ChunkCTD::read(&mut Cursor::new(&fileBytes))?;

    let mut vram = Vram::new();
    vram.add_ctd(&ctdFile);

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };
    let imageSampler = doc.add("samplers", object! {
        magFilter: NEAREST,
        minFilter: NEAREST,
        wrapS: CLAMP_TO_EDGE,
        wrapT: CLAMP_TO_EDGE,
    });
    // Grayscale view of vram
    let image = ltd::RGBAImage {
        width: 2048,
        height: 512,
        pixels: bytemuck::cast_slice(&vram.vram).iter().flat_map(|&p| [p, p, p, 255]).collect()
    };
    let textureInfo = rgbaImageToTextureInfo(image);
    let node = z_unlit_quad_with_texture_autosized(&mut doc, &textureInfo, Some(imageSampler));
    let baseScene = doc.add("scenes", object! {
        "nodes": [node],
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn convert_room_ctd(filesSource: &mut FilesSource, filename: &str, outputType: OutputType) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let fileBytes = filesSource.read_file(&filename)?;
    let ctd = mapctd::MapTextureData::read(&mut Cursor::new(&fileBytes))?;

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };
    let imageSampler = doc.add("samplers", object! {
        magFilter: NEAREST,
        minFilter: NEAREST,
        wrapS: CLAMP_TO_EDGE,
        wrapT: CLAMP_TO_EDGE,
    });
    let mut nodes = vec![];
    for tim in ctd.tims {
        let textureInfo = rgbaImageToTextureInfo(tim.to_rgba());
        nodes.push(z_unlit_quad_with_texture_autosized(&mut doc, &textureInfo, Some(imageSampler)));
    }
    let baseScene = doc.add("scenes", object! {
        "nodes": nodes
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn add_ltd_quads_to_output(doc: &mut gltf::Gltf, ltd: &ltd::LTD, sampler: usize) -> Vec<usize> {
    let mut result = vec![];
    if let Ok(image) = ltd.to_single_rgba() {
        result.push(z_unlit_quad_with_texture_autosized(doc, &rgbaImageToTextureInfo(image), Some(sampler)));
    } else {
        for image in ltd.images.iter() {
            // Grayscale, unknown association with cluts
            let textureInfo = rgbaImageToTextureInfo(ltd::RGBAImage {
                width: image.width as u32 * 2,
                height: image.height as u32,
                pixels: image.pixels.iter().cloned().flat_map(|p| [p, p, p, 255]).collect(),
            });
            result.push(z_unlit_quad_with_texture_autosized(doc, &textureInfo, Some(sampler)));
        }
        let textureInfo = rgbaImageToTextureInfo(ltd::RGBAImage {
            width: ltd.clut.width as u32,
            height: ltd.clut.height as u32,
            pixels: ltd.clut.clut.iter().cloned().flat_map(|p| ltd::psx16_to_rgba8888(p)).collect(),
        });
        result.push(z_unlit_quad_with_texture_autosized(doc, &textureInfo, Some(sampler)));
    }
    result
}

fn convert_ltd(filesSource: &mut FilesSource, filename: &str, outputType: OutputType) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let fileBytes = filesSource.read_file(&filename)?;
    let ltdFile = ltd::LTD::read(&mut Cursor::new(&fileBytes))?;

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };
    let imageSampler = doc.add("samplers", object! {
        magFilter: NEAREST,
        minFilter: NEAREST,
        wrapS: CLAMP_TO_EDGE,
        wrapT: CLAMP_TO_EDGE,
    });
    
    let nodes = add_ltd_quads_to_output(&mut doc, &ltdFile, imageSampler);

    let baseScene = doc.add("scenes", object! {
        "nodes": nodes
    });
    doc.root["scene"] = baseScene.into();
    
    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn convert_ltc(filesSource: &mut FilesSource, filename: &str, outputType: OutputType) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let fileBytes = filesSource.read_file(&filename)?;
    let ltcFile = ltc::LTC::read(&mut Cursor::new(&fileBytes))?;

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };
    let imageSampler = doc.add("samplers", object! {
        magFilter: NEAREST,
        minFilter: NEAREST,
        wrapS: CLAMP_TO_EDGE,
        wrapT: CLAMP_TO_EDGE,
    });
    let mut nodes = vec![];
    for ltd in ltcFile.images {
        nodes.extend(add_ltd_quads_to_output(&mut doc, &ltd, imageSampler));
    }
    let baseScene = doc.add("scenes", object! {
        "nodes": nodes
    });
    doc.root["scene"] = baseScene.into();
    
    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn convert_tim(filesSource: &mut FilesSource, filename: &str, outputType: OutputType) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let fileBytes = filesSource.read_file(&filename)?;
    let timFile = tim::TIM::read(&mut Cursor::new(&fileBytes))?;

    let textureInfo = rgbaImageToTextureInfo(timFile.to_rgba());

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };
    let imageSampler = doc.add("samplers", object! {
        magFilter: NEAREST,
        minFilter: NEAREST,
        wrapS: CLAMP_TO_EDGE,
        wrapT: CLAMP_TO_EDGE,
    });
    let node = z_unlit_quad_with_texture_autosized(&mut doc, &textureInfo, Some(imageSampler));
    let baseScene = doc.add("scenes", object! {
        "nodes": [node]
    });
    doc.root["scene"] = baseScene.into();
    
    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn convert_room_models(filesSource: &mut FilesSource, filename: &str, outputType: OutputType) -> Result<String> {
    anyhow::ensure!(matches!(outputType, OutputType::Gltf));

    let name = std::path::Path::new(&filename).file_stem().unwrap().to_str().unwrap();

    let fileBytes = filesSource.read_file(&filename)?;
    let room = mapbin::MapBin::read(&mut Cursor::new(&fileBytes))?;

    let mut texturesPath = std::path::PathBuf::from(&filename);
    texturesPath.set_extension("ctd");
    eprintln!("Checking for textures at: {:?}", texturesPath);
    let texturesFileBytes = filesSource.read_file(&texturesPath)?;
    let textures = mapctd::MapTextureData::read(&mut Cursor::new(&texturesFileBytes))?;

    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0",
    };
    doc.root["extensionsUsed"] = jzon::array! [ "KHR_materials_unlit" ];

    fn textureInfoFromTim(tim: &tim::TIM) -> TextureInfo {
        let image = tim.to_rgba();
        TextureInfo {
            size: (image.width as usize, image.height as usize),
            relativeName: None,
            pngBuffer: Some(image.to_png())
        }
    }

    let mut rootNodes = vec![];

    eprintln!("SUCCESS!");
    // std::fs::write("./test_layers_output.binary", &mapbin_decompressed.sections[7])?;
    let fallbackTextureInfo = make_magenta_checkerboard(256, 256, 16);
    for (sectionIndex, section) in room.sections.iter().enumerate() {
        if section.len() > 16 {
            let header = mapbin::MapBinModelHeader::read(&mut Cursor::new(&section))?;
            let potentialModelNumSections = u32::read(&mut Cursor::new(&section[12..]))?;
            eprintln!("Potential num model sections[{}]: {}", sectionIndex, potentialModelNumSections);
            if header.modelByteLength as usize == section.len() - 12 && (potentialModelNumSections == 5 || potentialModelNumSections == 6) {
                let model = readModel(&mut Cursor::new(&section[12..]))?;
                let roomModelTextureInfo = room_model_texture(&model, &header, &textures).map(|image| rgbaImageToTextureInfo(image));
                let textureInfo = roomModelTextureInfo.as_ref().unwrap_or(&fallbackTextureInfo);
                let mut animations = vec![];
                for (animIndex, anim) in model.animationData.animations.iter().enumerate() {
                    animations.push((format!("{}Anim{}", sectionIndex, animIndex), build_animation_buffer(&model.transformData.joints, anim, animIndex)));
                }
                rootNodes.push(add_model_to_output(&textureInfo, &model, outputType, &format!("{}{}", name, sectionIndex), &animations, &mut doc)?);
            }
        }
    }

    // Room layers
    {
        let backgroundPalettes = &room.sections[1];
        let layersSection = room.layers;

        eprintln!("Palettes byte length: {}", backgroundPalettes.len());
        eprintln!("Number of layers: {}", layersSection.numLayers);
        eprintln!("Number of tiles: {}", layersSection.tiles.len());

        #[derive(Hash, PartialEq, Eq, Clone)]
        struct TileTexture {
            timIndex: usize,
            paletteIndex: u8
        }
        #[derive(Hash, PartialEq, Eq, Clone)]
        struct TileMaterial {
            texture: usize,
            mode: u8,
        }
        let mut gltfTextures = std::collections::HashMap::<Option<TileTexture>, (usize, u32, u32)>::new();
        let mut materials = std::collections::HashMap::<TileMaterial, usize>::new();

        let tileSampler = doc.add("samplers", object! {
            magFilter: NEAREST,
            minFilter: NEAREST,
            wrapS: CLAMP_TO_EDGE,
            wrapT: CLAMP_TO_EDGE,
        });

        let mut layerNodes = vec![];

        let mut minLayerPos = vec3(INFINITY, INFINITY, INFINITY);
        let mut maxLayerPos = vec3(-INFINITY, -INFINITY, -INFINITY);
        let mut minLayerZ: f32 = 0.0;
        
        for layerNumber in 0..layersSection.numLayers as usize {
            #[derive(Copy, Clone, bytemuck::NoUninit)]
            #[repr(C)]
            struct TileVertex {
                position: Vector3,
                uv: Vector2,
            }
            let mut meshVertices = std::collections::HashMap::<usize, Vec<TileVertex>>::new();

            let layerTiles = &layersSection.tiles[layersSection.tileOffsets[layerNumber] as usize..layersSection.tileOffsets[layerNumber + 1] as usize];

            // For now, until the camera is totally figured out, assume a layer has uniform z so that it can be manipulated at the node level via translation
            let mut layerZ: f32 = 0.0;
            if !layerTiles.iter().all(|t| t.z == layerTiles[0].z) {
                eprintln!("WARNING: Layer {} has more than one z", layerNumber);
            }

            for tile in layerTiles {
                let tileUVRect = Rect {
                    x: tile.u,
                    y: 256 + tile.v as u16,
                    width: 8,
                    height: 16
                };
                let timIndex = textures.tims.iter().enumerate().find(|(_, t)| {
                    Rect { x: t.pixels.x, y: t.pixels.y, width: t.pixels.width, height: t.pixels.height }.contains(&tileUVRect)
                }).map(|(i, _)| i);
                let (texture, imageWidth, imageHeight) = *gltfTextures.entry(timIndex.map(|i| TileTexture { timIndex: i, paletteIndex: tile.palette })).or_insert_with_key(|tt| {
                    let name = match tt {
                        Some(tt) => format!("Layers-{}.{}", tt.timIndex, tt.paletteIndex),
                        None => format!("Layers-NotFound")
                    };
                    let (uri, width, height) = match tt {
                        Some(tt) => {
                            eprintln!("Using palette {} out of {} total", tt.paletteIndex, backgroundPalettes.len() / 512);
                            let pi = tt.paletteIndex / 8; // Why /8 here?
                            if (pi as usize) < backgroundPalettes.len() / 512 {
                                let clut: &[u16] = &(bytemuck::cast_slice::<u8, u16>(&backgroundPalettes))[(pi as usize * 256)..((pi + 1) as usize * 256)];
                                // TODO: don't convert the whole tim with the clut, keep track of all used subrects
                                let rgbaImage = textures.tims[tt.timIndex].to_rgba_with_clut(Some(clut));
                                (bytesToMimeURI("image/png", &rgbaImage.to_png()), rgbaImage.width, rgbaImage.height)
                            } else {
                                // HACK for a moment until I figure out how paletteIndex works
                                eprintln!("WARNING: Palette index out of range!");
                                (fallbackTextureInfo.uri(), fallbackTextureInfo.size.0 as u32, fallbackTextureInfo.size.1 as u32)
                            }
                        },
                        None => (fallbackTextureInfo.uri(), fallbackTextureInfo.size.0 as u32, fallbackTextureInfo.size.1 as u32)
                    };
                    let image = doc.add("images", object! {
                        "uri": uri,
                        "name": format!("{}-Image", name),
                    });
                    let texture = doc.add("textures", object! {
                        "source": image,
                        "sampler": tileSampler,
                        "name": format!("{}-Texture", name),
                    });
                    (texture, width, height)
                });
                let material = *materials.entry(TileMaterial { texture, mode: tile.mode }).or_insert_with_key(|m| {
                    eprintln!("Layer {}, semitranstexture {}, using mode {:x}", layerNumber, m.texture, m.mode);
                    // These numbers from utunnels' roomviewer DrawTile
                    let (godot_blend_mode, baseColorFactor, alphaMode) = match m.mode {
                        0x20 => ("mix", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
                        // (true, SEMI_TRANSPARENCY_MODE_MIX) => ("mix", vec4(1.0, 1.0, 1.0, 0.5), "BLEND"),
                        0x28 | 0xa8 => ("add", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
                        0xb0 => ("subtract", vec4(1.0, 1.0, 1.0, 1.0), "MASK"),
                        _ => ("add", vec4(0.25, 0.25, 0.25, 1.0), "MASK"),
                    };
                    doc.add("materials", object! {
                        "name": name.to_string() + "SemiTransparentMaterial",
                        "pbrMetallicRoughness": {
                            "metallicFactor": 0,
                            "baseColorFactor": baseColorFactor.as_ref().as_slice(),
                            "baseColorTexture": {
                                "index": m.texture,
                            }
                        },
                        "alphaMode": alphaMode,
                        "extras": { "blend_mode": godot_blend_mode, "info_cc_tile_mode": format!("0x{:x}", m.mode) },
                        "extensions": {
                            "KHR_materials_unlit": {}
                        }
                    })
                });

                let vertices = meshVertices.entry(material).or_insert_with(Vec::new);
                let timX = timIndex.map(|ti| textures.tims[ti].pixels.x).unwrap_or(0);
                let timY = timIndex.map(|ti| textures.tims[ti].pixels.y).unwrap_or(0);
                let x = tile.x as f32;
                let y = tile.y as f32;
                layerZ = tile.z as f32 * -1.0 / 625.0; // ARBITRARY SCALE DOWN, THIS IS WRONG
                let z = 0.0; // tile.z as f32;
                let u = ((tile.u as u16 - timX) as f32 * 2.0) / imageWidth as f32;
                let v = ((256 + tile.v as u16 - timY) as f32) / imageHeight as f32;
                let uw = 16.0 / imageWidth as f32;
                let vh = 16.0 / imageHeight as f32;
                let tileScale = vec3(1.0, -1.0, -1.0); // Negate Y to flip upright, have to switch winding order below
                vertices.extend([
                    TileVertex { position: vec3(x, y, z) * tileScale, uv: vec2(u, v) },
                    TileVertex { position: vec3(x, y + 16.0, z) * tileScale, uv: vec2(u, v + vh) },
                    TileVertex { position: vec3(x + 16.0, y, z) * tileScale, uv: vec2(u + uw, v) },

                    TileVertex { position: vec3(x + 16.0, y, z) * tileScale, uv: vec2(u + uw, v) },
                    TileVertex { position: vec3(x, y + 16.0, z) * tileScale, uv: vec2(u, v + vh) },
                    TileVertex { position: vec3(x + 16.0, y + 16.0, z) * tileScale, uv: vec2(u + uw, v + vh) },
                ]);
            }

            let mut primitives = vec![];
            for (material, vertices) in meshVertices {
                let mut minPosition = vertices[0].position;
                let mut maxPosition = vertices[0].position;
                for v in &vertices {
                    minPosition = minPosition.min(v.position);
                    maxPosition = maxPosition.max(v.position);
                }
                minLayerZ = minLayerZ.min(layerZ);
                minLayerPos = minLayerPos.min(minPosition);
                maxLayerPos = maxLayerPos.max(maxPosition);
                let vertexBytes: &[u8] = bytemuck::cast_slice(&vertices);
                let vertexBuffer = doc.add("buffers", object! {
                    "uri": bytesToURI(&vertexBytes),
                    "byteLength": vertexBytes.len(),
                });
                let vertexBufferView = doc.add("bufferViews", object! {
                    "buffer": vertexBuffer,
                    "byteLength": vertexBytes.len(),
                    "target": ARRAY_BUFFER,
                    "byteStride": std::mem::size_of::<TileVertex>(),
                });
                let positionAccessor = doc.add("accessors", object! {
                    "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(TileVertex, position), "type": "VEC3", "componentType": FLOAT, "count": vertices.len(),
                    "min": [minPosition.x, minPosition.y, minPosition.z],
                    "max": [maxPosition.x, maxPosition.y, maxPosition.z]
                });
                let uvAccessor = doc.add("accessors", object! {
                    "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(TileVertex, uv), "type": "VEC2", "componentType": FLOAT, "count": vertices.len()
                });
                primitives.push(object! {
                    "attributes": {
                        "POSITION": positionAccessor,
                        "TEXCOORD_0": uvAccessor,
                    },
                    "mode": 4,
                    "material": material
                });
            }
            let mesh = doc.add("meshes", object! {
                "primitives": primitives,
                "name": name.to_string() + "Mesh"
            });
            layerNodes.push(doc.add("nodes", object! {
                "name": format!("Layer{}", layerNumber),
                "mesh": mesh,
                "translation": [0.0, 0.0, layerZ],
            }));
        }

        // Add a black backdrop because some rooms' base layers use additive blending
        let z = 0.0; // minLayerZ - 0.1;
        let backdropNode = z_unlit_quad_with_texture(&mut doc, &make_black_texture(32, 32), vec3(minLayerPos.x, minLayerPos.y, z), vec3(maxLayerPos.x, maxLayerPos.y, z), None, "Backdrop");
        layerNodes.push(backdropNode);
        doc.root["nodes"][backdropNode]["translation"] = jzon::array! [ 0.0, 0.0, minLayerZ - 0.1 ];

        eprintln!("Room bounds: {} to {}", minLayerPos, maxLayerPos);

        rootNodes.push(doc.add("nodes", object! {
            "name": "Background",
            "children": layerNodes,
        }));
    }

    // Walk mesh
    {
        eprintln!("Num walk triangles: {}, Num walk vertices: {}", room.walkMeshTriangles.triangles.len(), room.walkMeshVertices.vertices.len());

        #[derive(Default, Copy, Clone, bytemuck::NoUninit)]
        #[repr(C)]
        struct Vertex {
            position: Vector3,
        }

        let vertices: Vec<Vertex> = room.walkMeshVertices.vertices.iter().map(|v| Vertex { position: formats::cc_position(v.x, v.y, v.z) }).collect();
        let indices: Vec<u16> = room.walkMeshTriangles.triangles.iter().flat_map(|t| [t.index1, t.index3, t.index2]).collect(); // Reverse winding

        let vertexBytes: &[u8] = bytemuck::cast_slice(&vertices);
        let indicesBytes: &[u8] = bytemuck::cast_slice(&indices);

        let mut minPosition = vertices[0].position;
        let mut maxPosition = vertices[0].position;
        for v in &vertices {
            minPosition = minPosition.min(v.position);
            maxPosition = maxPosition.max(v.position);
        }

        let indexBuffer = doc.add("buffers", object! {
            "uri": bytesToURI(indicesBytes),
            "byteLength": indicesBytes.len(),
        });
        let indexBufferView = doc.add("bufferViews", object! {
            "buffer": indexBuffer,
            "byteLength": indicesBytes.len(),
            "target": ELEMENT_ARRAY_BUFFER,
        });
        let indexAccessor = doc.add("accessors", object! {
            "bufferView": indexBufferView, "byteOffset": 0, "componentType": UNSIGNED_SHORT, "count": indices.len(), "type": "SCALAR"
        });

        let vertexBuffer = doc.add("buffers", object! {
            "uri": bytesToURI(&vertexBytes),
            "byteLength": vertexBytes.len(),
        });
        let vertexBufferView = doc.add("bufferViews", object! {
            "buffer": vertexBuffer,
            "byteLength": vertexBytes.len(),
            "target": ARRAY_BUFFER,
            "byteStride": std::mem::size_of::<Vertex>(),
        });
        let positionAccessor = doc.add("accessors", object! {
            "bufferView": vertexBufferView, "byteOffset": std::mem::offset_of!(Vertex, position), "type": "VEC3", "componentType": FLOAT, "count": vertices.len(),
            "min": [minPosition.x, minPosition.y, minPosition.z],
            "max": [maxPosition.x, maxPosition.y, maxPosition.z]
        });

        let material = doc.add("materials", object! {
            "name": "WalkMeshMaterial",
            "pbrMetallicRoughness": {
                "baseColorFactor": [1.0, 1.0, 1.0, 0.2],
            },
            "alphaMode": "BLEND",
            "doubleSided": true,
        });
        let mesh = doc.add("meshes", object! {
            "primitives": [
                {
                    "attributes": {
                        "POSITION": positionAccessor,
                    },
                    "indices": indexAccessor,
                    "material": material,
                    "mode": 4,
                }
            ],
            "name": format!("{}-WalkMesh", name)
        });

        // The extremely confusing consequences of formats::CC_AXIS_FLIP, my hubris
        fn vec3fixed(x: i16, y: i16, z: i16) -> Vector3 {
            vec3(x as f32, -y as f32, -z as f32) / 4096.0
        }
        let c = &room.camera;
        let cameraBasisX = vec3fixed(c.m11, c.m21, c.m31);
        let cameraBasisY = vec3fixed(c.m12, c.m22, c.m32);
        let cameraBasisZ = vec3fixed(c.m13, c.m23, c.m33);
        let cameraTransform = Matrix3::from_cols(cameraBasisX, cameraBasisY, cameraBasisZ);
        let translation = formats::cc_position(c.tx, c.ty, c.tz);

        let matrix = Matrix4::from_diagonal(vec4(1.0, -1.0, -1.0, 1.0)) * Matrix4::from_mat3_translation(cameraTransform, translation).inverse();
        let camera = doc.add("cameras", object! {
            "type": "perspective",
            "name": "Room3DCamera",
            "perspective": {
                "yfov": (45.0 * std::f32::consts::PI / 180.0), // Arbitrary for now
                "znear": 0.1, // Arbitrary for now
            }
        });
        let cameraNode = doc.add("nodes", object! {
            "name": "Room3DCamera",
            "camera": camera,
            "matrix": matrix.as_ref().as_slice(),
            "extras": {
                "cameraMystery": {
                    "unknown1": c.unknown1,
                    "xLow": (c.maybe_xMin),
                    "xHigh": (c.maybe_xMax),
                    "yLow": (c.maybe_yMin),
                    "yHigh": (c.maybe_yMax),
                    "unknown2": c.unknown2,
                }
            },
        });

        eprintln!("Mysteries: {}, {}, {}, {}, {}, {}", c.unknown1, c.maybe_xMax, c.maybe_xMin, c.maybe_yMax, c.maybe_yMin, c.unknown2);

        let mut dbgInfo = jzon::array![];
        for face in room.walkMeshTriangles.triangles.iter() {
            let avgPos = (vertices[face.index1 as usize].position + vertices[face.index2 as usize].position + vertices[face.index3 as usize].position) / 3.0;
            dbgInfo.push(jzon::object! {
                "position": avgPos.as_ref().as_slice(),
                "info": face.info
            }).unwrap();
        }

        rootNodes.push(doc.add("nodes", object! {
            "name": format!("{}-WalkMesh", name),
            "mesh": mesh,
            "children": [ cameraNode ],
            "extras": { "walkmesh-debug": dbgInfo }
        }));
    }

    let baseScene = doc.add("scenes", object! {
        "nodes": rootNodes
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn convert_model(filesSource: &mut FilesSource, filename: &str, outputType: OutputType, extra_anim_filenames: &[String], extra_prd_anim_filenames: &[(String, String)]) -> Result<String> {
    let name = std::path::Path::new(&filename).file_stem().unwrap().to_str().unwrap();
    let fileBytes = filesSource.read_file(&filename)?;
    let model = readModel(&mut Cursor::new(&fileBytes))?;

    let textureInfo = find_texture(filesSource, &filename).unwrap_or_else(|_| make_magenta_checkerboard(256, 256, 16));

    let mut animations: Vec<(String, AnimationBuffer)> = vec![];

    for (animIndex, anim) in model.animationData.animations.iter().enumerate() {
        animations.push((format!("Anim{}", animIndex), build_animation_buffer(&model.transformData.joints, anim, animIndex)));
    }

    // Try loading attack animations (at0 from the prd)
    match read_attack_animations(filesSource, &std::path::Path::new(&filename), &model) {
        Ok(anims) => animations.extend(anims),
        Err(err) => eprintln!("Reading attack anims failed: {:?}", err),
    }

    // Try loading more specified animations
    for extra_anim_file in extra_anim_filenames {
        let anims = read_bin_animations(filesSource, extra_anim_file.as_ref(), &model)?; // Propagate failure here because it was explicitly requested
        for (animIndex, anim) in anims.into_iter().enumerate() {
            animations.push((format!("{}{}", extra_anim_file, animIndex), anim));
        }
    }
    // Try loading more specified prd animations
    for (prd_filename, prd_subfilename) in extra_prd_anim_filenames {
        let anims = read_prd_animations(filesSource, prd_filename.as_ref(), prd_subfilename, &model)?; // Propagate failure here because it was explicitly requested
        for (animIndex, anim) in anims.into_iter().enumerate() {
            animations.push((format!("{}/{}{}", prd_filename, prd_subfilename, animIndex), anim));
        }
    }
    
    let mut doc = gltf::Gltf::new();
    doc.root["asset"] = object! {
        "generator": "CC2GLTF",
        "version": "2.0"
    };

    doc.add("extensionsUsed", "GODOT_single_root".into());

    let skeletonNode = add_model_to_output(&textureInfo, &model, outputType, &name, &animations, &mut doc)?;

    let baseScene = doc.add("scenes", object! {
        "nodes": [
            skeletonNode,
        ]
    });
    doc.root["scene"] = baseScene.into();

    Ok(jzon::stringify_pretty(doc.root, 4))
}

fn main() -> Result<()> {
    std::assert_eq!(std::mem::align_of::<Transform>(), 2);

    let mut filename = None;
    let mut outputType = OutputType::Gltf;
    let mut zipFile = None;

    enum InputType {
        // Models
        Model, // .obj files
        Weapon, // Weapon .bin files
        Room, // Room .bin files
        Mesh, // Same as a weapon geometry, or prd-embedded mesh or battlefield mesh, and surprsingly weapon.kmd files
        Prd, // Archive full of meshes, battlefield meshes, and textures
        // Textures
        Tim,
        RoomCtd, // Room .ctd files
        Ltd,
        Ltc,
        EffectCtd,
    }
    use InputType::*;

    let mut inputType = None;

    let mut dumpPrdPath = None;

    let mut args = std::env::args().skip(1);
    let mut extra_anim_filenames = vec![];
    let mut extra_prd_anim_filenames = vec![];
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {println!("{}", HELP_STR); return Ok(());},
            "--format=obj" => outputType = OutputType::Obj,
            "--format=gltf" => outputType = OutputType::Gltf,
            "--zip" => zipFile = args.next(),
            "--add-bin-anims" => extra_anim_filenames.push(args.next().unwrap()),
            "--add-prd-anims" => extra_prd_anim_filenames.push((args.next().unwrap(), args.next().unwrap())),
            "--type=weapon" => inputType = Some(InputType::Weapon),
            "--type=model" => inputType = Some(InputType::Model),
            "--type=room" => inputType = Some(InputType::Room),
            "--type=tim" => inputType = Some(InputType::Tim),
            "--type=roomctd" => inputType = Some(InputType::RoomCtd),
            "--type=ltd" => inputType = Some(InputType::Ltd),
            "--type=ltc" => inputType = Some(InputType::Ltc),
            "--type=mesh" => inputType = Some(InputType::Mesh),
            "--type=prd" => inputType = Some(InputType::Prd),
            "--type=effectctd" => inputType = Some(InputType::EffectCtd),
            "--dump-prd" => dumpPrdPath = Some(args.next().unwrap()),
            _ if arg.starts_with("-") => eprintln!("Unknown option: {}", arg),
            _  if filename.is_none() => filename = Some(arg),
            _ => eprintln!("Too many arguments! Ignoring {}", arg),
        }
    }

    if std::env::args().len() == 1 {
        eprintln!("{}", HELP_STR);
        return Ok(());
    }

    let rawzipArchive = if let Some(rawzipFilename) = zipFile {
        let mut rawzipBuffer = vec![0u8; rawzip::RECOMMENDED_BUFFER_SIZE];
        let archive = rawzip::ZipArchive::from_file(std::fs::File::open(rawzipFilename).unwrap(), &mut rawzipBuffer).unwrap();
        Some((rawzipBuffer, archive))
    } else {
        None
    };
    let mut filesSource = FilesSource { zip_archive: rawzipArchive };

    let filename = filename.unwrap();
    eprintln!("Reading {}", filename);

    if let Some(dump_path) = dumpPrdPath {
        dump_prd(&mut filesSource, &filename, &dump_path)?;
        return Ok(());
    }

    let typesToTry = match inputType {
        Some(t) => vec![t],
        None => match std::path::Path::new(&filename).extension().and_then(|ext| ext.to_str()) {
            Some("obj") => vec![Model],
            Some("bin") => vec![Weapon, Room],
            Some("ctd") => vec![EffectCtd, RoomCtd],
            Some("tim") => vec![Tim],
            Some("ltd") => vec![Ltd],
            Some("ltc") => vec![Ltc],
            Some("kmd") => vec![Mesh],
            Some("prd") => vec![Prd],
            _ => vec![Prd, Model, Weapon, Room, Tim, RoomCtd, Ltd, Ltc, Mesh, EffectCtd],
        }
    };

    let mut typesToTry = typesToTry.into_iter();
    let mut typeToTry = typesToTry.next().unwrap();
    loop {
        let result = match typeToTry {
            InputType::Room => convert_room_models(&mut filesSource, &filename, outputType),
            InputType::Weapon => convert_weapon_model(&mut filesSource, &filename, outputType),
            InputType::Model => convert_model(&mut filesSource, &filename, outputType, &extra_anim_filenames, &extra_prd_anim_filenames),
            InputType::Tim => convert_tim(&mut filesSource, &filename, outputType),
            InputType::RoomCtd => convert_room_ctd(&mut filesSource, &filename, outputType),
            InputType::Ltd => convert_ltd(&mut filesSource, &filename, outputType),
            InputType::Ltc => convert_ltc(&mut filesSource, &filename, outputType),
            InputType::Mesh => convert_mesh(&mut filesSource, &filename, outputType),
            InputType::Prd => convert_prd(&mut filesSource, &filename, outputType),
            InputType::EffectCtd => convert_effect_ctd(&mut filesSource, &filename, outputType),
        };
        match result {
            Ok(result) => {
                println!("{}", result);
                return Ok(());
            },
            Err(e) => match typesToTry.next() {
                Some(t) => typeToTry = t,
                None => return Err(e),
            }
        }
    }
}

fn bytesToMimeURI(mime_type: &'static str, bytes: &[u8]) -> String {
    use base64::prelude::*;
    let mut result = String::new();
    result.push_str("data:");
    result.push_str(mime_type);
    result.push_str(";base64,");
    result.push_str(&BASE64_STANDARD.encode(bytes));
    result
}
fn bytesToURI(bytes: &[u8]) -> String {
    bytesToMimeURI("application/octet-stream", bytes)
}