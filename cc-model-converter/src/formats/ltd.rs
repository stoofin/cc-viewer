use binread::{BinRead,FilePtr32};
use crate::CurPos;

#[derive(BinRead)]
pub struct Clut {
    pub unknown1: u32,
    pub x: u16,
    pub y: u16,
    pub unknown2: u32,
    pub width: u16, // In vram width
    pub height: u16, 

    #[br(count = width as u32 * height as u32)] // as u32 probably unnecessary
    pub clut: Vec<u16>,
}

#[derive(BinRead)]
pub struct Image {
    pub unknown1: u32,
    pub texturePageX: u16,
    pub texturePageY: u16,
    pub x: u16,
    pub y: u16,
    pub width: u16, // vram width, # of u16's
    pub height: u16,
    #[br(count = (width as u32) * 2 * (height as u32))] // as u32 otherwise a 128 * 2 * 256  == 65536, which overflows a u16
    pub pixels: Vec<u8>,
}

#[derive(BinRead)]
pub struct LTD {
    pub baseOffset: CurPos,
    pub num_objects: u32,
    #[br(offset = baseOffset.0)]
    pub clut: FilePtr32<Clut>,
    #[br(offset = baseOffset.0)]
    #[br(count = num_objects - 1)]
    pub images: Vec<FilePtr32<Image>>,
}

pub fn psx16_to_rgba8888(p: u16) -> [u8; 4] {
    let r = p & 0b1_1111;
    let g = (p >> 5) & 0b1_1111;
    let b = (p >> 10) & 0b1_1111;
    // let a = p >> 15;

    fn u5_to_u8(u5: u16) -> u8 {
        return (u5 as f32 * 255.0 / 31.0).round() as u8;
    }

    let transparent = p == 0;

    return [
        u5_to_u8(r),
        u5_to_u8(g),
        u5_to_u8(b),
        if transparent { 0 } else { 255 },
    ];
}

pub struct RGBAImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RGBAImage {
    pub fn blit(&mut self, other: &RGBAImage, offset_x: u32, offset_y: u32) {
        for y in 0..other.height {
            for x in 0..other.width {
                for s in 0..4 {
                    self.pixels[(((offset_y + y) * self.width + offset_x + x) * 4 + s) as usize] = other.pixels[((y * other.width + x) * 4 + s) as usize];
                }
            }
        }
    }
    pub fn to_png(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = vec![];
        eprintln!("Converting to png: {:?}x{:?}", self.width, self.height);
        let mut encoder = png::Encoder::new(&mut buffer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&self.pixels).unwrap();
        drop(writer);

        buffer
    }
}

impl LTD {
    pub fn to_rgba(&self, image: &Image) -> RGBAImage {
        // HACK Assume images wider than 128 are two textures side by side, (texture page size is 128 u8 (64 u16) x 256)
        // with the left one using palette 0, and the right one using palette 1
        let mut pixelBytes = vec![];
        for y in 0..image.height {
            for x in 0..image.width * 2 {
                let index = image.pixels[y as usize * image.width as usize * 2 + x as usize];
                let clut_base = if image.x * 2 + x > 128 && self.clut.height > 1 { 256 } else { 0 };
                pixelBytes.extend(psx16_to_rgba8888(self.clut.clut[clut_base + index as usize]));
            }
        }
        RGBAImage {
            width: image.width as u32 * 2,
            height: image.height as u32,
            pixels: pixelBytes
        }
    }

    pub fn to_single_rgba(&self) -> anyhow::Result<RGBAImage> {
        use std::cmp::{min, max};
        if self.images.len() == 1 {
            return Ok(self.to_rgba(&self.images[0]));
        }
        eprintln!("Attempting to combine {} images into one texture!", self.images.len());
        // For things like battle/kurage/kurage.ltd|obj, they have multiple textures with UVs that indicate the textures should be stacked
        let mut xLow = self.images[0].x;
        let mut xHigh = self.images[0].x + self.images[0].width;
        let mut yLow = self.images[0].y;
        let mut yHigh = self.images[0].y + self.images[0].height;
        for image in self.images.iter().skip(1) {
            xLow = min(xLow, image.x);
            xHigh = max(xHigh, image.x + image.width);
            yLow = min(yLow, image.y);
            yHigh = max(yHigh, image.y + image.height);
        }
        let totalWidth = xHigh as u32 - xLow as u32;
        let totalHeight = yHigh as u32 - yLow as u32;
        anyhow::ensure!(totalWidth <= 256); // Allow for two horizontal texture pages, (used by e.g. akaoni.obj)
        anyhow::ensure!(totalHeight <= 256);
        let mut combinedTexture = RGBAImage {
            width: totalWidth * 2,
            height: totalHeight,
            pixels: vec![0u8; totalWidth as usize * 2 * totalHeight as usize * 4],
        };
        for image in self.images.iter() {
            let otherRgba = self.to_rgba(image);
            combinedTexture.blit(&otherRgba, image.x as u32 * 2, image.y as u32);
        }
        Ok(combinedTexture)
    }
}
