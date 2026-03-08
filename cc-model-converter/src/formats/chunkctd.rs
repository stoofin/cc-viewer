use binread::{BinRead};

// https://www.chronocompendium.com/Term/Egfx.html

#[derive(BinRead)]
pub struct ChunkHeader {
    pub sectionType: u32,
    pub x1: u16,
    pub y1: u16,
    pub x2: u16,
    pub y2: u16,
    pub vramWidth: u16,
    pub unknown3: [u16; 3],
    pub sectionsInFile: u32,
    pub sectionSectorCount: u32,
    #[br(align_after = 2048)]
    #[br(count = sectionSectorCount)]
    pub rowsInSector: Vec<u16>,
}

#[derive(BinRead)]
pub struct Sector {
    #[br(count = 1024)]
    pub data: Vec<u16>,
}

pub struct ChunkSection {
    pub header: ChunkHeader,
    pub sectors: Vec<Sector>,
}

pub struct ChunkCTD {
    pub sections: Vec<ChunkSection>
}

impl BinRead for ChunkCTD {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(reader: &mut R, _options: &binread::ReadOptions, _args: Self::Args) -> binread::BinResult<Self> {
        let mut sections = vec![];
        let mut sectionIndex = 0;
        loop {
            let header = ChunkHeader::read(reader)?;
            let sectionsInFile = header.sectionsInFile;
            let mut sectors = vec![];
            for _ in 0..header.sectionSectorCount {
                sectors.push(Sector::read(reader)?);
            }
            sections.push(ChunkSection {
                header,
                sectors,
            });
            sectionIndex += 1;
            if sectionIndex == sectionsInFile {
                break;
            }
        }
        Ok(ChunkCTD {
            sections
        })
    }
}