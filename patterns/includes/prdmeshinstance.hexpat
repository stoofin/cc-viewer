enum CommandType : u8 {
    MESH = 0x05,
    TRANSLATION = 0x08,
    SCALE = 0x12,
    ROTATION = 0x19,
    COLORS = 0x20
};

const str known = "ffff00";
fn commandColor(ref auto c) {
    match (c) {
        (CommandType::MESH): return known;
        (CommandType::TRANSLATION): return known;
        (CommandType::SCALE): return known;
        (CommandType::ROTATION): return known;
        (CommandType::COLORS): return known;
        (_): return "ff0000";
    }
    //if (c == 0x05) return "ff0000";
    //return "ffff00";
};

fn fmt_command(u8 c) {
    return std::format("0x{:x}", c);
};

struct MeshCommand {
    u8 command [[format("fmt_command"), color("ffff00")]]; // color(commandColor(this))]];// [[color("ffff00")]];
    u8 lengthTimesTwo [[color("cc9900")]];
    u8 args[lengthTimesTwo / 2 - 2] [[color("005599")]];
};

import std.mem;
import std.sys;

struct MeshInstance {
    u32 zero1; // Changes at runtime
    u32 zero2;
    u16 runtimeCounter; // Monotonically increasing counter
    u16 spawnRate; // Time between spawns
    u32 runtimeTimeUntilNextSpawn; // Time until next spawn at runtime, starts at spawnRate and ticks down
    u32 oh1oh4; // Looks like a start command but probably not?
    u32 zero;
    u16 oh1oh4Command; // Actual start
    std::assert(oh1oh4Command == 0x0401, "Invalid start command");
    
    MeshCommand commands[while(std::mem::read_unsigned($, 2, std::mem::Endian::Little) != 0x0400)];
    
    u16 oh0oh4Command; // end
    std::assert(oh0oh4Command == 0x0400, "Invalid end command");
    
    u8 b_padding[(4 - $ % 4) % 4];
    
    // u32 just before the name also changed from zeroes to data at runtime
    
    // just before end is 6 bytes of rotation: s16 rotationX, rotationY, rotationZ;
    // ends with u16 == 00 04
    // Uses 'bb' aka 62 62 as padding to align back to 4 bytes
};
MeshInstance mi @ 0x00;
//MeshInstance meshinstance_at_0x7EA34612DF20 @ 0x7EA34612DF24;
//MeshInstance meshinstance_at_0x7EA34612E170 @ 0x7EA34612E170;
//MeshInstance meshinstance_at_0x7EA34612D710 @ 0x7EA34612D710;
//MeshInstance meshinstance_at_0x7EA34612D7CC @ 0x7EA34612D7CC;
//MeshInstance meshinstance_at_0x7EA34612DC64 @ 0x7EA34612DC64;
// MeshInstance meshinstance_at_0x7753A03179E4 @ 0x7753A03179E4;