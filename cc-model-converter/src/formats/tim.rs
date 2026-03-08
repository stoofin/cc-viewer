use binread::BinRead;
use crate::formats::ltd::{psx16_to_rgba8888, RGBAImage};

#[derive(BinRead, Clone)]
pub struct PixelsData {
    pub len: u32,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    #[br(count = (width as u32) * (height as u32))] // as u32 to avoid potential overflow on 256 * 256, texture page size is 128x256 but you never know
    pub pixels: Vec<u16>,
}

#[derive(BinRead, Clone)]
pub struct TIM {
    pub magic: u32,
    #[br(assert(magic == 0x10, "TIM magic number missing?"))]
    pub flags: u32,

    #[br(if((flags & 8) != 0))]
    pub clut: Option<PixelsData>,
    
    pub pixels: PixelsData,
}

impl TIM {
    pub fn to_rgba_with_clut(&self, clut: Option<&[u16]>) -> RGBAImage {
        const BPP4: u32 = 0b00;
        const BPP8: u32 = 0b01;
        const BPP16: u32 = 0b10;
        const BPP24: u32 = 0b11;

        let mode = self.flags & 0b11;
        
        let wshort = self.pixels.width;
        let widthPixels = match mode {
            BPP4 => wshort as u32 * 4,
            BPP8 => wshort as u32 * 2,
            BPP16 => wshort as u32,
            BPP24 => wshort as u32 * 4 / 3,
            _ => unreachable!(),
        };

        let pixels = match (mode, clut) {
            (BPP4, Some(clut)) => {
                let bytes: &[u8] = bytemuck::cast_slice(&self.pixels.pixels);
                bytes.iter().map(|&twoIndices| {
                    let indexA = twoIndices & 0b1111;
                    let indexB = twoIndices >> 4;
                    let colorA = psx16_to_rgba8888(clut[indexA as usize]);
                    let colorB = psx16_to_rgba8888(clut[indexB as usize]);
                    [
                        colorA[0], colorA[1], colorA[2], colorA[3],
                        colorB[0], colorB[1], colorB[2], colorB[3],
                    ]
                }).flatten().collect()
            },
            (BPP8, Some(clut)) => {
                let bytes: &[u8] = bytemuck::cast_slice(&self.pixels.pixels);
                bytes.iter().map(|&index| {
                    psx16_to_rgba8888(clut[index as usize])
                }).flatten().collect()
            },
            (BPP16, None) => {
                self.pixels.pixels.iter().map(|&color| {
                    psx16_to_rgba8888(color)
                }).flatten().collect()
            },
            (BPP8, None) => { // For e.g. displaying map/mapbin/*.ctd on its own, since the palette is in the .bin
                let bytes: &[u8] = bytemuck::cast_slice(&self.pixels.pixels);
                bytes.iter().map(|&index| {
                    [index, index, index, 255]
                }).flatten().collect()
            },
            _ => unimplemented!()
        };

        RGBAImage {
            width: widthPixels,
            height: self.pixels.height as u32,
            pixels
        }
    }
    pub fn to_rgba(&self) -> RGBAImage {
        self.to_rgba_with_clut(self.clut.as_ref().map(|c| c.pixels.as_slice()))
    }

    pub fn to_png(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = vec![];
        eprintln!("Converting to png, {:?}x{:?}", self.pixels.width, self.pixels.height);

        let RGBAImage { width: widthPixels, height: heightPixels, pixels: rgbaData } = self.to_rgba();
        let mut encoder = png::Encoder::new(&mut buffer, widthPixels, heightPixels);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();

        dbg!(widthPixels * self.pixels.height as u32);
        dbg!(self.pixels.pixels.len());
        dbg!(rgbaData.len());
        writer.write_image_data(&rgbaData).unwrap();
        drop(writer);

        buffer
    } 
}
