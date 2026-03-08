use binread::{BinRead};

#[derive(BinRead)]
pub struct Entry {
    pub zero: u32,
    pub pos_x: i16,
    pub pos_y: i16,
    pub pos_z: i16,
    pub somethingSmaller: u16,
    pub focus_x: i16,
    pub focus_y: i16,
    pub focus_z: i16,
    pub usuallyTrailingZero: u16,
}

#[derive(BinRead)]
pub struct CameraPath {
    pub zero1: u32,
    pub mystery1: u16,
    pub mystery2: u16,
    pub zero2: u16,
    pub mystery3: u16,
    pub zero3: u32,
    pub count: u32,

    #[br(count = count)]
    pub entries: Vec<Entry>
}