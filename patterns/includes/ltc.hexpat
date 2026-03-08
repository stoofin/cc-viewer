import * from ltd as ATIM;

struct ATIMPtr {
    u32 offset;
    ATIM atim @ addressof(parent) + offset;
};

// https://www.chronocompendium.com/Term/Cpt.html
struct CPT {
    u32 numObjects;
    ATIMPtr atims[numObjects] [[inline]];
    u32 endOfCpt;
};

CPT cpt @ 0;