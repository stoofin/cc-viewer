import std.core;
import std.sys;
import std.io;
import * from tim as TIM;
import * from weapongeo as WeaponGeometry;

// https://www.chronocompendium.com/Term/Weap.html

struct WeaponModel {
    u32 timOffset;
    u32 geoOffset;
    TIM tim @ timOffset;
    WeaponGeometry geo @ geoOffset;
};

struct PaddingFile {
    u32 p;
    std::assert(p == 0x77777720, "Not padding at the end of the weapon?");
};
struct PaddingPtr {
    u32 offset;
    PaddingFile @ offset;
};

// https://www.chronocompendium.com/Term/Cpt.html
struct CPT {
    u32 numObjects;
    WeaponModel model[numObjects / 2 - 1];
    PaddingPtr endPadding[2];
    u32 endOfCpt;
};

CPT cpt @ 0;