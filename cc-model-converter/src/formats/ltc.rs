use binread::{BinRead,FilePtr32};
use crate::formats::ltd::LTD;

#[derive(BinRead)]
pub struct LTC {
    pub num_objects: u32,
    #[br(count = num_objects)]
    pub images: Vec<FilePtr32<LTD>>,
    pub endOfCptOffset: u32,
}