import std.io;
import std.core;
import type.magic;
import * from model as Model;
import * from tim as TIM;
import * from weapongeo as Geo;
import * from prdmeshinstance as MeshInstance;

struct ScriptCommand {
    u8 command [[color("ffff00")]];
    u8 lengthTimesTwo [[color("cc9900")]];
    u8 args[lengthTimesTwo / 2 - 2] [[color("009955")]];
};
struct PrdScript {
    u32 zeros[5];
    ScriptCommand commands[while($ < addressof(parent) + 12 + parent.actualFileLength)];
};

struct CameraPathEntry {
    u32 zero;
    s16 posX; // Data seems to relate to its corresponding field in the next sub entry
    s16 posY;
    s16 posZ;
    u16 somethingSmaller;
    s16 focusX;
    s16 focusY;
    s16 focusZ; // Probably not angles, seen them exceed 4096
    u16 usuallyTrailingZero; // Sometimes non-zero, e.g. bg_00.prd @ 0x275e
};
struct CameraPath {
    u32 zero1;
    u16 mystery1; // Maybe duration? Usually between 100 and 600
    u16 mystery2; // Usually 0, seen 1 and 2
    u16 zero2;
    u16 mystery3; // Always 256?
    u32 zero3;
    u32 count;
    CameraPathEntry subs[count];
};

struct TimInfoSubrecord {
    u8 data[12];
};
struct TimInfoRecord {
    u8 count;
    u24 zero;
    TimInfoSubrecord subrecords[count];
};

struct TimInfoRecordPtr {
    u16 offset;
    TimInfoRecord record @ addressof(parent) + offset;
};

// https://www.chronocompendium.com/Term/Timinfo.html
struct TimInfo {
    u16 zero;
    u8 recordCount;
    u8 unknown1;
    u16 textureX;
    u16 textureY;
    u16 clutX;
    u16 clutY;
    TimInfoRecordPtr records[recordCount];
    // Either padding here to re-align to 4 bytes, or always trailing 00 00
};

// https://www.chronocompendium.com/Term/Drp.html
enum FileType : u8 {
    DRP = 0x01,
    MESH = 0x02,
    TIMINFO = 0x03,
    TIM = 0x04,
    MINST = 0x05,
    UNKNOWN1 = 0x07,
    MESHINSTANCE = 0x0A,
    MDL = 0x0B, // RDE .obj
    CAMERA_PATH = 0x0c,
    UNKNOWN4 = 0x10,
    BATTLEFIELD_MESH = 0x12,
    TIMINFO_LENS_LIGHT_MAYBE = 0x15,
    MSEQ = 0x16,
    UNKNOWN5 = 0x18,
    ANIM = 0x19, // animation as in the kind embedded in .obj
    UNKNOWN6 = 0x1a,
    UNKNOWN7 = 0x24,
    LZSS = 0x25,
};

using DRP;

struct FileHeader {
    u32 zero;
    char name[4];
    FileType fileType;
    //std::print("{} : {}", fileType, u8(fileType));
    str nameStr = std::format("{}.{}", name, fileType); // Workaround for [[format]], otherwise it can't see a child's name for some reason
    u24 fileLength;
    u32 actualFileLength = fileLength / 16 [[export]];
    std::print("[{}] {}.{} with length: {}", std::core::array_index(), name, fileType, actualFileLength);
    match (fileType)
    {
        (FileType::MDL): Model model;
        (FileType::TIM): TIM tim;
        (FileType::MESH): Geo geometry; // Seems to work, but some of the data is sus
        (FileType::DRP): DRP subDRP [[inline]];
        (FileType::BATTLEFIELD_MESH): Geo battlefield_geo;
        (FileType::MESHINSTANCE): MeshInstance mesh_instance;
        (FileType::UNKNOWN4): PrdScript script;
        (FileType::CAMERA_PATH): CameraPath camera_path;
        (FileType::TIMINFO): TimInfo timInfo;
        (_): u8 unknownFileContentsBytes[actualFileLength] [[color("ff0000")]];
    }
} [[format("format_File")]];
fn format_File(ref auto f) { return f.nameStr; };

struct FilePtr {
    u32 offset;
    FileHeader file @ addressof(parent) + offset [[inline]];
} [[format("format_FilePtr")]];
fn format_FilePtr(ref auto f) { return f.file; };

struct DRP {
    type::Magic<"drp\0"> magic;
    u32 zero;
    u16 numFiles;
    u16 zero2;
    u16 actualNumFiles = numFiles / 64 [[export]];
    FilePtr files[actualNumFiles] [[inline]];
};

DRP drp @ 0;