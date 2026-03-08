struct Ptr {
    u32 offset;
    u8 something @ offset;
};

// https://www.chronocompendium.com/Term/Cpt.html
struct CPT {
    u32 numObjects;
    Ptr ptrs[numObjects];
    u32 endOfCpt;
};

CPT cpt @ 0x0;