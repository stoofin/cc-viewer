import std.mem;

struct RoomTile {
    s16 x;
    s16 y;
    u16 u;
    u8 v;
    u8 p;
    u8 order;
    u8 mode;
    u16 z;
};

struct RoomLayers {
    u32 numLayers;
    u32 offsets[numLayers + 1];
    RoomTile tiles[offsets[numLayers] + 1];
};

RoomLayers rl @ 0x00;