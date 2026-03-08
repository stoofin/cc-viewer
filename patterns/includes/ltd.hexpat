// https://www.chronocompendium.com/Term/Atim.html

// https://docs.werwolv.net/imhex/views/pattern-data

import type.color;

fn uq(auto v) { // unquantize
    return u32(float(v) * 255f / 31f);
};

fn to_u32(auto v) {
    return uq(v.r) + (uq(v.g) << 8) + (uq(v.b) << 16) + 0xFF000000;
};

fn as_u32(auto v) {
    return std::format("{:#x}", v);
};

bitfield RGBA5551 {
    r : 5;
    g : 5;
    b : 5;
    a : 1;
} [[sealed, transform("to_u32"), format("as_u32")]];

struct Clut {
   u32 unknown1; // Only seen 01 11 00 00
   u16 x;
   u16 y;
   u32 unknown2; // Only seen 0
   u16 width;  // Should be 0x0100 = 256
   u16 height; // Only ever seen 0x0001 or 0x0002
   RGBA5551 pixels[width * height];
};
struct ClutPtr {
    u32 offset;
    Clut clut @ offset [[inline]];  
};

struct Image {
   u32 unknown1; // seen 01 11 00 00 (palette-like textures, 16x1) and 00 11 00 00 (normal textures)
   u16 texturePageX;
   u16 texturePageY;
   u16 x; // Within the texture page.
   u16 y; // If above is texturePage.y, this is the y coordinate within that page. See battle/../kurage.ltd
   u16 width; // Width in vram u16s (so for 8-bit palette mode it's 2*width pixels wide)
   u16 height;
   u8 pixels[width * 2 * height];
} [[hex::visualize("bitmap", pixels, width * 2, height, parent.parent.clut.clut.pixels)]];
struct ImagePtr {
    u32 offset;
    Image image @ offset;  
};

struct ATIM {
    u32 numObjects;
    ClutPtr clut;
    ImagePtr images[numObjects - 1];
} [[hex::visualize("bitmap", images[0].image.pixels, images[0].image.width * 2, images[0].image.height, clut.clut.pixels)]];

ATIM atim @ 0;