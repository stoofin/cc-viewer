import * from model as Model;

struct ModelSection {
    u32 modelByteLength; // nextOffset - thisOffset - 12
    u16 texturePageX;
    u16 texturePageY;
    u16 clutX;
    u16 clutY;
    Model model;
};

struct SomethingPtr {
    u32 offset;
    u32 something @ offset; // Sometimes 'sszl'
    u32 potentialModelSections @ offset + 12;
    if (potentialModelSections == 5 || potentialModelSections == 6) {
        ModelSection modelSection @ offset;
    }
};

struct WalkTriangle {
    u16 indices[3];
    s16 neighboringFaces[3];
    u16 probablyBitfield;
} [[static]];
struct WalkTrianglesSection {
    u32 num;
    WalkTriangle triangles[num];
};

struct WalkVertex {
    s16 x;
    s16 y;
    s16 z;
    s16 w;
} [[static]];
struct WalkVerticesSection {
    u32 numTimes8;
    WalkVertex vertices[numTimes8 / 8];
};

struct Camera {
    // 3x3 Matrix, row-major, fixed point 4.12
    s16 m11;
    s16 m12;
    s16 m13;
    s16 m21;
    s16 m22;
    s16 m23;
    s16 m31;
    s16 m32;
    s16 m33;
    
    // Translation component
    s16 tx;
    s16 ty;
    s16 tz;
    
    // Not kept around at runtime
    s16 unknown1;
    s16 unknown2;
    s16 unknown3; // often ~ -unknown2
    s16 unknown4;
    s16 unkonwn5; // often ~ -unknown4
    s16 unknown6;
    
    u32 count;
    u32 unknowns[count];
};

// https://www.chronocompendium.com/Term/Cpt.html
struct CPT {
    u32 numObjects;
    SomethingPtr somethings[numObjects];
    u32 endOfCpt;
    
    Camera @ somethings[0].offset;
    WalkTrianglesSection @ somethings[4].offset;
    WalkVerticesSection @ somethings[5].offset;
};

CPT cpt @ 0;
// For alu_a01f.bin
/*
ModelSection m1 @ 0x10ac0;
ModelSection m2 @ 0x1154c;
ModelSection m3 @ 0x15610;
ModelSection m4 @ cpt.somethings[14].offset;
ModelSection m5 @ cpt.somethings[15].offset;
ModelSection m6 @ cpt.somethings[16].offset;
ModelSection m7 @ cpt.somethings[17].offset;
*/