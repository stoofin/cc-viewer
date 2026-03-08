import std.core;


fn fmt_FaceType(ref auto f) {
    if (f.isTextured) {
        if (f.isQuad) {
            return "Textured Quad";    
        } else {
            return "Textured Tri";
        }
    } else {
        if (f.isQuad) {
            return "Untextured Quad";    
        } else {
            return "Untextured Tri";
        
        }
    }
};

bitfield FaceType {
    MAYBE_isRawTexture: 1;
    MAYBE_isSemiTransparent: 1;
    
    isTextured: 1;
    isQuad: 1;
    // MAYBE_isGouraudShaded: 1;
    vertexColorPerVertex : 1;
    
    MAYBE_command: 3; // always 0b001?
    unk1: 8;
} [[format("fmt_FaceType")]];

struct Normal {
    s8 x, y, z;
    u8 unknown;
};

struct Vertex {
    s16 x, y, z;
    s16 index;
};

struct UV {
    u8 u, v;
} [[inline]];

struct VertexColor {
    u8 x, y, z, w;
};


struct Face {
    if (parent.faceType.isTextured)
        UV textureUV[parent.faceType.isQuad ? 4 : 3];
    else
        VertexColor vertexColor;
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
    if (!parent.faceType.isQuad && !parent.faceType.isTextured)
        s16 wasted;
};

struct FaceChunk {
    FaceType faceType;
    s16 numFaces;
    u32 start;
    Face faces[numFaces] @ addressof(parent.parent.parent) + parent.faceOffset + start;
};

struct VertexGroupOneJoint {
    s16 numVertices;
    s16 joint1;
};
struct VertexGroupTwoJoints {
    s16 numVertices;
    s16 unknown1;
    s16 joint1;
    s16 weight1;
    s16 joint2;
    s16 weight2;
};

struct FaceGroup {
    u32 faceOffset;
    u32 vertexOffset;
    u32 normalOffset;
    
    u32 numOneJointVertexGroups;
    u32 oneJointVerticesOffset;
    VertexGroupOneJoint oneJointVertexGroups[numOneJointVertexGroups];
    
    u32 numTwoJointVertexGroups;
    u32 twoJointVerticesOffset;
    VertexGroupTwoJoints twoJointVertexGroups[numTwoJointVertexGroups];
    
    u32 numChunks;
    FaceChunk chunks[numChunks];
    
    Vertex oneJointVertices[sumNumVertices(oneJointVertexGroups)] @
        addressof(parent.parent) + vertexOffset + oneJointVerticesOffset;
        
    Vertex twoJointVertices[sumNumVertices(twoJointVertexGroups) * 2] @
        addressof(parent.parent) + vertexOffset + twoJointVerticesOffset;
        
    Normal normals[std::core::member_count(oneJointVertices) + std::core::member_count(twoJointVertices)] @
        addressof(parent.parent) + normalOffset;
};

fn sumNumVertices(ref auto vs) {
    u32 len = std::core::member_count(vs);
    u32 sum = 0;
    for (u32 i = 0, i < len, i += 1) {
        sum += vs[i].numVertices;
    }
    return sum;
};

struct FaceGroupRef {
    u32 offset;
    FaceGroup group @ offset + addressof(parent) [[inline]];
};

struct FaceData {
    u32 numFaceGroups;
    FaceGroupRef faceGroups[numFaceGroups];
    
    u8 unknown1;
    u8 unknown2;
    u8 unknown3;
    u8 unknown4;
    u32 unknown5;
};

struct TransformAngTra {
    s16 dAngleX;
    s16 dAngleY;
    s16 dAngleZ;
    
    s16 dPositionX;
    s16 dPositionY;
    s16 dPositionZ;
};

struct Transform {
    s32 parentIndex;
    
    TransformAngTra;
    
    s16 unknown1;
    s16 unknown2;
};

struct TransformData {
    u32 numTransforms;
    Transform transforms[numTransforms];
};

struct AttachmentPoint {
    s16 x, y, z;
    u16 joint;
};
bitfield ColorInfoFlags {
    isInvisible: 1;
    isSemiTransparent: 1; // vs opaque (1 for e.g. harle's shear... pants?)
    semiTransparencyMode: 2; // only matters if isSemiTransparent
        // https://psx-spx.consoledev.net/graphicsprocessingunitgpu/#semi-transparency
        // 0b00 => B/2+F/2 aka 50% opacity alpha blending
        // 0b01 => B+F aka addition
        // 0b10 => B-F akd subtraction
        // 0b11 => B+F/4 aka addition at 25% opacity
    
    unknown1: 1; // 0 for most things, 1 for kid's skirt, harle's pants
    unknown234: 3; // Supposedly reflectiveness and texture offset/scaling something?
};
struct ColorInfo {
    // https://problemkaputt.de/psx-spx.htm#gpurenderingattributes
    u8 r, g, b; // Where 128 is neutral, as per above source
    ColorInfoFlags flags;
};

struct WeaponTransform {
    TransformAngTra [[inline]];
    s16 attachmentPoint;
    s16 unknown1;
};

// https://www.chronocompendium.com/Term/Mdl.html
struct Section3 {
    AttachmentPoint attachmentPoints[32];
    ColorInfo colorInfos[16];
    WeaponTransform weaponTransforms[2];
};

struct JointKeyframe {
    s16 dX;
    s16 dY;
    s16 dZ;
} [[format("JointKeyframe_format")]];

fn JointKeyframe_format(ref auto j) {
    return std::format("{}, {}, {}", j.dX, j.dY, j.dZ);
};

import std.io;
fn round_up_to_next_multiple(u32 v, u32 divisor) {
    u32 result = (v - 1) / divisor + 1;
    return result;
};

// Helper function for bit counting
fn popcount(ref auto vs) {
    u32 len = std::core::member_count(vs);
    u32 count = 0;
    for (u32 i = 0, i < len, i += 1) {
        u32 flags = vs[i];
        for (u32 b = 0, b < 32, b += 1) {
            count += flags & 1;
            flags >>= 1;
        }
    }
    return count;
};

import std.sys;

fn AnimationFilePtr_format(ref auto a) {
    return std::format("{} frames", a.anim.numFrames);
};

struct AnimationKeyframes {
    std::assert($ == addressof(parent) + parent.frameOffsets[std::core::array_index() + 1], "Keyframe offset doesn't match!'");
    JointKeyframe keyframes[parent.jointBitFlagsPopcount] [[inline]];
};

struct AnimationFile {
    u32 numFrames; // Counts initialTransforms as a frame
    u32 numTransforms = parent.parent.numTransforms;
    u32 jointBitFlags[round_up_to_next_multiple(numTransforms * 2, 32)];
    u8 jointBitFlagsPopcount = popcount(jointBitFlags) [[export]]; // u8 because u32 triggers a bug
    u32 frameOffsets[numFrames];
    std::assert($ == addressof(this) + frameOffsets[0], "Miscalculated metadata length?");
    TransformAngTra initialTransforms[numTransforms];
    AnimationKeyframes keyframes[numFrames - 1];
    if ($ % 4 == 2) {
        u16 alignPadding;
        std::assert(alignPadding == 0x7777, "Animation alignment padding not 0x7777?");
    }
};

struct AnimationFilePtr {
    u32 offset;
    AnimationFile anim @ addressof(parent) + offset [[inline]];
} [[format("AnimationFilePtr_format")]];

// https://www.chronocompendium.com/Term/Anim.html
struct AnimationSection<auto nTransforms> {
    u32 numTransforms = nTransforms;
    u32 numSubfiles;
    AnimationFilePtr anims[numSubfiles];
    u32 endOfCptOffset;
};

struct Section5Block {
    u32 probably0or16;
    u8 paddingV[12];
};

struct Section5Ptr {
    u32 offset;
    Section5Block block @ addressof(parent) + offset [[inline]];
};

struct Section5 {
    u32 numPointers;
    Section5Ptr pointers[numPointers];
    u32 endOfSectionPointer;
};

// ImHex chokes on this cyclic import, allocates memory in a loop until it's murdered
// import * from prd as PRD;

struct Model {
    u32 numSections;
    u32 sectionOffsets[numSections];
    u32 modelLength;
    
    FaceData faceData @ sectionOffsets[0];
    TransformData transformData @ sectionOffsets[1];
    Section3 section3 @ sectionOffsets[2];
    AnimationSection<transformData.numTransforms> animationSection @ sectionOffsets[3];
    if (sectionOffsets[4] != 0) Section5 section5 @ sectionOffsets[4]; // In e.g. battle/effects/syoukan/tgolem.prd[66] it will have numSections == 6 but the final two offsets will be 0
    if (numSections == 6) {
        // PRD section6 @ sectionOffsets[5]; // Had issues with recursion here, disabled for now for ImHex
    }
};

u32 numberOfJoints in;
struct Main {
    if (numberOfJoints == 0)
        Model model @ 0x0;
    else
        AnimationSection<numberOfJoints> anims @ 0x0;
};

Main main @ 0x0;