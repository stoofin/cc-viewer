// https://www.chronocompendium.com/Term/Egfx.html

struct Sector {
    u8 sectorBytes[2048];
};

// egfx? https://www.chronocompendium.com/Term/Egfx.html
struct CTD {
    u32 sectionType;
    u16 num1;
    u16 num2;
    u16 num3;
    u16 num4;
    u16 vramWidth;
    u16 unknown1;
    u16 unknown2;
    u16 unknown3;
    u32 sectionsInFile;
    u32 sectionSectorCount;
    u16 rowsInSector[sectionSectorCount];
    u8 remainderOfHeader[2020 - sectionSectorCount * 2];
    
    Sector data[sectionSectorCount];
    
    u32 sectionNumber = parent.sectionNumber + 1;
    if (sectionNumber < sectionsInFile)
        CTD nextHeader;
    
};

struct CTDRoot {
    u32 sectionNumber = 0;
    CTD ctd;
};

CTDRoot ctd @ 0x00;