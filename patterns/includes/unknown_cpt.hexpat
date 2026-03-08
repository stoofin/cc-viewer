struct MysteryDataPtr {
    u32 offset;
    u32 mystery @ offset;
};

struct CPT {
    u32 numFiles;
    MysteryDataPtr files[numFiles];
    u32 endOfCpt;
};

CPT cpt @ 0x00;