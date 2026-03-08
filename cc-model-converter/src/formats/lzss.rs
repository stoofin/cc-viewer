use bitstream_io::{BitRead, BitReader, BigEndian};

pub fn decompress_lzss(bytes: &[u8]) -> Vec<u8> {
    const BUFFER_LEN: usize = 4096;
    let mut output = vec![];
    let mut buffer = vec![0u8; BUFFER_LEN];
    let mut cursor = std::io::Cursor::new(bytes);
    let mut bits = BitReader::endian(&mut cursor, BigEndian);

    let mut pos = 0;
    while bits.position_in_bits().map(|pos| pos < (bytes.len() * 8) as u64).unwrap_or(false) {
        match bits.read_bit().unwrap_or(false) {
            false => {
                let mut offset = bits.read_unsigned::<12, u16>().unwrap_or(0) as usize + BUFFER_LEN - 1;
                let len = 2 + bits.read_unsigned::<4, u16>().unwrap_or(0) as usize;
                for _ in 0..len {
                    let to_copy = buffer[offset % BUFFER_LEN];
                    buffer[pos % BUFFER_LEN] = to_copy;
                    output.push(to_copy);
                    pos += 1;
                    offset += 1;
                }
            },
            true => { 
                let byte: u8 = bits.read_unsigned::<8, u8>().unwrap_or(0);
                buffer[pos % BUFFER_LEN] = byte;
                output.push(byte);
                pos += 1;
            }
        }
    }

    output
}