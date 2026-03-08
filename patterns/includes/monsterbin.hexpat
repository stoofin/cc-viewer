// Enter your pattern code here and click on the Play button
// below to execute your code. The results will then be visible
// in the Pattern Data view.
// 
// More information can be found in the documentation.
// 
// 
// Simple example:
// 
// import std.io;
// 
// struct Pattern {
//     u32 int;
//     float floating_point;
// };
// 
// Pattern my_pattern @ 0x00;
// std::print("0x{:08X}", my_pattern.int);

import std.sys;

struct MonsterBinInfo {
    u8 unknown;
    u8 innate;
    u8 unknown2;
    u8 growthTable;
    u8 unknown3[17];
    u8 gender;
    u16 hp;
    u8 atk;
    u8 def;
    u8 hit;
    u8 evd;
    u8 mag;
    u8 mdef;
    u8 statusEffectLength;
    u8 stamina;
    u16 equippedWeapon;
    u16 equippedArmor;
    u16 equippedAccessory1;
    u16 equippedAccessory2;
    u16 equippedAccessory3;
    u8 unknown4;
    u8 doppelgangId;
    u8 unknown5;
    u8 modelId;
    u8 actionPoints;
    u8 weaponModel;
    u8 elementGrid[16];
    u16 elements[48];
    u8 treasureDrop;
    u8 unknown6;
    u16 goldDropped;
    u8 mediumHit;
    u8 heavyHit;
    u8 doppelgangable;
    u8 unknown7[7];
    u16 monsterType;
    std::print("Model: {}", modelId - 0x40);
};

import std.io;
import std.mem;
struct MonsterBin {
    MonsterBinInfo infos[while(!std::mem::eof())];
};

MonsterBin mbin @ 0x00;