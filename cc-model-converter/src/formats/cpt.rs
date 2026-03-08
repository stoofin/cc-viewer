use binread::{BinRead, ReadOptions, BinResult};
use std::io::{Read, Seek, SeekFrom};
use super::lzss;

#[derive(BinRead)]
#[br(import(length: u32))]
pub struct CPTEntry {
    #[br(count = length)]
    pub contents: Vec<u8>,
}

pub struct CPT {
    pub entries: Vec<CPTEntry>,
}

#[derive(BinRead)]
struct CPTRaw {
    num_objects: u32,
    #[br(count = num_objects + 1)]
    offsets: Vec<u32>,
}

impl BinRead for CPT {
    type Args = ();
    fn read_options<R: Read + Seek>(reader: &mut R, _ro: &ReadOptions, _args: Self::Args) -> BinResult<Self> {
        let cpt_raw = CPTRaw::read(reader)?;
        let restore_position = reader.stream_position()?;
        let mut entries = vec![];
        for window in cpt_raw.offsets.windows(2) {
            let &[offset, offsetEnd] = window else { unreachable!(); };
            if offsetEnd < offset {
                return Err(binread::Error::AssertFail { pos: reader.stream_position()?, message: format!("CPT offsets decreasing {} > {}!", offset, offsetEnd) });
            }
            reader.seek(SeekFrom::Start(offset as  u64))?;
            entries.push(CPTEntry::read_args(reader, (offsetEnd - offset,))?);
        }
        reader.seek(SeekFrom::Start(restore_position))?;
        Ok(CPT {
            entries
        })
    }
}


pub struct CPTWithCompression {
    pub sections: Vec<Vec<u8>>
}

impl BinRead for CPTWithCompression {
    type Args = ();
    
    fn read_options<R: std::io::Read + std::io::Seek>(reader: &mut R, _options: &binread::ReadOptions, _args: Self::Args) -> binread::BinResult<Self> {
        let cpt = CPT::read(reader)?;
        let mut result = vec![];

        for entry in cpt.entries.into_iter() {
            let contents = entry.contents;
            let contents = if contents.len() > 12 && contents[0] == 0x73 && contents[1] == 0x73 && contents[2] == 0x7A && contents[3] == 0x6C {
                // lzss, decompress
                let decompressed_len = u32::read(&mut std::io::Cursor::new(&contents[4..8]))?;
                let _maybe_checksum = u32::read(&mut std::io::Cursor::new(&contents[9..13]))?; // Always 0x88??????

                let mut decompressed =  lzss::decompress_lzss(&contents[12..]);
                eprintln!("Decompressed {} bytes to {} bytes, expected {}", contents.len() - 12, decompressed.len(), decompressed_len);
                if decompressed.len() > decompressed_len as usize {
                    eprintln!("Truncating!");
                    decompressed.resize(decompressed_len as usize, 0);
                }
                decompressed
            } else {
                contents
            };
            result.push(contents);
        }

        Ok(CPTWithCompression {
            sections: result
        })
    }

}