use binread::{BinRead};
use super::tim::TIM;
use super::cpt::CPTWithCompression;

pub struct MapTextureData {
    pub tims: Vec<TIM>,
}

impl BinRead for MapTextureData {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(reader: &mut R, _options: &binread::ReadOptions, _args: Self::Args) -> binread::BinResult<Self> {
        let ctd = CPTWithCompression::read(reader)?;
        let tims = (ctd.sections.into_iter().map(|bytes| TIM::read(&mut std::io::Cursor::new(&bytes))).collect::<Result<Vec<TIM>, _>>())?; 
        Ok(MapTextureData {
            tims
        })
    }
}