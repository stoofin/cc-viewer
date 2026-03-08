import std.sys;
import std.io;

bitfield FaceType {
    MAYBE_isRawTexture: 1;
    MAYBE_isSemiTransparent: 1;
    
    isTextured: 1;
    isQuad: 1;
    // MAYBE_isGouraudShaded: 1;
    vertexColorPerVertex : 1;
    
    MAYBE_command: 3; // always 0b001?
    unk1: 8;
};

// https://www.chronocompendium.com/Term/Mesh.html
bitfield ClutInfo {
    clutX: 6;
    clutY: 10;
};

// https://web.archive.org/web/20160817104617/http://psx.rules.org/gpu.txt
bitfield TextureInfo {
    texturePageX: 4; // texture pages are 64x256 in vram, vram is 1024x512 (u16's), there are 16x2 = 32 texture pages, representable by 4+1 bits
    texturePageY: 1;
    trasparencyMode: 2; // 0b00=(F+B)/2, 0b01=F+B, 0b10=B-F, 0b11=B+F/4
    mode: 2; // 7,8 (corresponds to tim mode, 0 = 4bpp, 1 = 8bpp, 2 = 16bpp)
    
    dither: 1;
    drawToDisplayAreaAllowed: 1;
    applyMaskBitToDrawnPixels: 1;
    doNotDrawToPixelsWithSetMaskBit: 1;
    unknown: 3;
};

struct Vertex {
    s16 x, y, z;
    u16 index;
} [[static]];

struct UV {
    u8 u, v;
};

struct VertexColor {
    u8 x, y, z, w; // psx 128 white
};

struct Face3 {
    if (parent.faceType.vertexColorPerVertex) {
        VertexColor vertexColor[3];
    } else {
        VertexColor vertexColor;
    }

    if (parent.faceType.isTextured) {
        UV uv1;
        ClutInfo clutInfo;
        UV uv2;
        TextureInfo textureInfo;
        UV uv3;
    }
    
    s16 indices[3];

    if (!parent.faceType.isTextured)
        s16 alignnmentPadding;
};

struct Face4 {
    if (parent.faceType.vertexColorPerVertex) {
        VertexColor vertexColor[4];
    } else {
        VertexColor vertexColor;
    }

    if (parent.faceType.isTextured) {
        UV uv1;
        ClutInfo clutInfo;
        UV uv2;
        TextureInfo textureInfo;
        UV uv3;
        UV uv4;
    }
    
    s16 indices[4];
};

/*
struct Face {
    VertexColor vertexColor[parent.faceType.isQuad ? 4 : 3];
    
    UV uv1;
    ClutInfo clutInfo;
    UV uv2;
    TextureInfo textureInfo;
    UV uv3;
    if (parent.faceType.isQuad)
        UV uv4;
        
    s16 indices[parent.faceType.isQuad ? 4 : 3];
    
    if (parent.faceType.isQuad) {
        s16 pointer1 = indices[0] / 16 [[export]];
        s16 pointer2 = indices[1] / 16 [[export]];
        s16 pointer3 = indices[2] / 8 [[export]];
        s16 pointer4 = indices[3] / 8 [[export]];
        s16 flags1 = indices[0] & 0xf [[export]];
        s16 flags2 = indices[1] & 0xf [[export]];
        s16 flags3 = indices[2] & 0x7 [[export]];
        s16 flags4 = indices[3] & 0x7 [[export]];
    } else {
        s16 pointer1 = indices[0] / 8 [[export]];
        s16 pointer2 = indices[1] / 16 [[export]];
        s16 pointer3 = indices[2] / 16 [[export]];
        s16 flags1 = indices[0] & 0x7 [[export]];
        s16 flags2 = indices[1] & 0xf [[export]];
        s16 flags3 = indices[2] & 0xf [[export]];
    }
};
*/

struct Group {
    FaceType faceType;
    u16 numFaces;
    u32 faceOffsetStart;
    if (faceType.isQuad)
        Face4 faces[numFaces] @ addressof(parent) + faceOffsetStart;
    else
        Face3 faces[numFaces] @ addressof(parent) + faceOffsetStart;
    // Face faces[numFaces] @ addressof(parent) + faceOffsetStart;
};

struct WeaponModel {
    u32 unknown1; // Seen: 01 00 00 80
    u16 numVertices;
    u16 unknown2;
    u32 vertexPoolOffset;
    u32 facePoolOffset;
    u32 unknown3;
    u32 numGroups;
    Group groups[numGroups];
    std::assert($ == facePoolOffset, "Where the faces at?");
    
    Vertex vertices[numVertices] @ vertexPoolOffset;
};

WeaponModel wmodel @ 0x00;