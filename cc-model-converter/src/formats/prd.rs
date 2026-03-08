use binread::{BinRead,FilePtr32};
use crate::CurPos;

#[allow(non_camel_case_types)]
#[derive(BinRead, Debug)]
#[br(repr(u8))]
#[repr(u8)]
pub enum FileType {
    DRP = 0x01, // recursive PRD
    MESH = 0x02, // parseable with weapbin::WeaponGeometry
    TIMINFO = 0x03,
    TIM = 0x04,
    MINST = 0x05,
    UNKNOWN1 = 0x07,
    UNKNOWN2 = 0x0A,
    MDL = 0x0B, // RDE .obj, parseable with crate::readModel
    CAMERA_PATH = 0x0c,
    UNKNOWN4 = 0x10,
    BATTLEFIELD_MESH = 0x12,
    TIMINFO_LENS_LIGHT_MAYBE = 0x15,
    MSEQ = 0x16,
    UNKNOWN5 = 0x18, // appears many times at the start of Van's at0.prd "rhp{0,1,2,3}", "lhp{0,1,2,3}"
    ANIM = 0x19, // animation as in the kind embedded in .obj, parseable with animation::AnimationData if you know the number of joints
    UNKNOWN6 = 0x1a,
    UNKNOWN7 = 0x24, // appears in e.g. ten_b07f.prd, very short 8 byte file
    LZSS = 0x25,
}

#[derive(BinRead)]
pub struct PrdFile {
    start: CurPos,

    pub zero: u32,
    pub name: [u8; 4],
    pub file_type: FileType,
    pub file_length_low: u16,
    pub file_length_high: u8,
    #[br(calc(((file_length_high as u32) << 16) | file_length_low as u32))]
    pub file_length: u32,
    #[br(count = file_length / 16)]
    pub contents: Vec<u8>,

    end: CurPos,
}

#[derive(BinRead)]
pub struct PRD {
    basePos: CurPos,

    pub magic: u32, // 'drp\0'
    #[br(assert(magic == 0x00707264))] 
    pub zero: u32,
    pub numFilesTimes64: u16,
    pub zero2: u16,

    #[br(offset = basePos.0)]
    #[br(count = numFilesTimes64 / 64)]
    pub files: Vec<FilePtr32<PrdFile>>,

    end: CurPos,
}

impl PRD {
    /** Include the trailing \0 if the filename has one, the bytes must match exactly and str is not nul-terminated. */
    pub fn first_file_matching_name<'a, 'b>(&'a self, name: &'b str) -> Option<&'a PrdFile> {
        for file in self.files.iter() {
            if file.name == name.as_bytes() {
                return Some(file);
            }
        }
        None
    }
    pub fn sanity_check(&self) -> Result<(), anyhow::Error> {
        if self.files[0].start.0 == self.end.0
            && self.files.iter().zip(self.files.iter().skip(1)).all(|(a, b)| a.end.0 == b.start.0)
            // && self.files[self.files.len() - 1].end ==  {
        {
                Ok(())
        } else {
            Err(anyhow::anyhow!("Failed sanity check"))
        }
    }
}