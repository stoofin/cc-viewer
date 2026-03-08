// https://www.chronocompendium.com/Term/2541.html

import std.io;

struct TechInfo {
    u8 bitmask;
    u24 unknown1;
    u8 targetingBitmask;
    u8 elementColorBitmask;
    u8 elementLevel;
    u8 elementType;
    u16 strength;
    u16 healingStrength;
    u8 randomFactor;
    u8 unknown2;
    u16 unknown3;
    u16 effectIndex; // can get from this to drp
    u8 unknown4;
    u8 message;
    u8 techCharacter1; // Unfortunately these are all 255 in the file. Maybe at runtime they're replaced by something useful
    u8 techCharacter2;
    u8 techCharacter3;
    u8 unclear;
    u32 unclear2;
    u32 unclear3;
    u16 secondTechForDoubleTechIndex;
    u16 secondTechForTripleTechIndex;
    u16 thirdTechForTripleTechIndex;
    u16 resultingDoubleTechIndex;
    u16 resultingTripleTechIndex;
    u16 summonRelated;
    
    if (techCharacter3 != 255) std::print(techCharacter3);
} [[static]];

import std.mem;

struct TechInfos {
    TechInfo infos[while(!std::mem::eof())];
};

TechInfos ti @ 0x00;