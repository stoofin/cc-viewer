// https://www.chronocompendium.com/Term/Tim.html

import type.magic;

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
} [[static, sealed, transform("to_u32"), format("as_u32")]];

fn to_grayscale(auto v) {
    return v.v | (v.v << 8) | (v.v << 16) |  0xFF000000;
};
struct GrayscaleU8 {
    u8 v;
} [[static, sealed, transform("to_grayscale")]];

fn to_u4(ref auto b) {
    return b.lo;
};
// Using this will truncate the value to the first 4 bits, lets you visualize 4bpp lookup but at half-width
bitfield u8DisplayHack {
    lo: 4;
    hi: 4;
} [[static, sealed, transform("to_u4")]];

struct PixelsHeader {
    u32 len;
    u16 x;
    u16 y;
    u16 width;
    u16 height;
};

struct Pixels4WithoutClut {
    PixelsHeader header [[inline]];
    u8 textureData[header.width * 2 * header.height];
} [[hex::visualize("bitmap", textureData, header.width * 2, header.height)]];
struct Pixels8WithoutClut {
    PixelsHeader header [[inline]];
    GrayscaleU8 textureData[header.width * 2 * header.height];
} [[hex::visualize("bitmap", textureData, header.width * 2, header.height)]];
struct Pixels16WithoutClut {
    PixelsHeader header [[inline]];
    RGBA5551 textureData[header.width * header.height];
} [[hex::visualize("bitmap", textureData, header.width, header.height)]];
struct Pixels24WithoutClut {
    PixelsHeader header [[inline]];
    u24 textureData[header.width * 2 / 3 * header.height];
};

struct Pixels4WithClut {
    PixelsHeader header [[inline]];
    u8DisplayHack textureData[header.width * 2 * header.height];
} [[hex::visualize("bitmap", textureData, header.width * 2, header.height, parent.clut.textureData)]];
struct Pixels8WithClut {
    PixelsHeader header [[inline]];
    u8 textureData[header.width * 2 * header.height];
} [[hex::visualize("bitmap", textureData, header.width * 2, header.height, parent.clut.textureData)]];
struct Pixels16WithClut {
    PixelsHeader header [[inline]];
    RGBA5551 textureData[header.width * header.height];
};
struct Pixels24WithClut {
    PixelsHeader header [[inline]];
    u24 textureData[header.width * 2 / 3 * header.height];
};


struct TIM {
    u32 magic;
    std::assert(magic == 0x10, "Failed magic number check");
    u32 flags;
    u8 bpp = flags & 3 [[export]];
    bool has_clut = flags & 8 != 0 [[export]];
    if (has_clut) Pixels16WithoutClut clut;
    if (has_clut) {
        match (bpp) {
            (0b00): Pixels4WithClut image;
            (0b01): Pixels8WithClut image;
            (0b10): Pixels16WithClut image;
            (0b11): Pixels24WithClut image;
        }
    } else {
        match (bpp) {
            (0b00): Pixels4WithoutClut image;
            (0b01): Pixels8WithoutClut image;
            (0b10): Pixels16WithoutClut image;
            (0b11): Pixels24WithoutClut image;
        }
    }
};

TIM tim @ 0;