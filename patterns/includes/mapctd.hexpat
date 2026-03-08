import * from tim as TIM;

struct TIMPtr {
    u32 offset;
    u32 magic @ offset;
    if (magic == 0x10) TIM tim @ offset;
    if (magic == 0x6c7a7373) u8 startOfLZss @ offset;
};

// https://www.chronocompendium.com/Term/Cpt.html
struct CPT {
    u32 numSections;
    TIMPtr section[numSections];
    u32 endOfCPT;
};

CPT cpt @ 0x0;