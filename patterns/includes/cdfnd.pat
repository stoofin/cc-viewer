// Enter your pattern code here and click on the Play button
// below to execute your code. The results will then be visible
// in the Pattern Data view.
// 
// More information can be found in the documentation.
// 
// 
// Simple example:
// 
// import std.io;
// 
// struct Pattern {
//     u32 int;
//     float floating_point;
// };
// 
// Pattern my_pattern @ 0x00;
// std::print("0x{:08X}", my_pattern.int);

import std.mem;

struct CDFndEntry {
    char name[64];  
};
struct CDFnd {
    CDFndEntry file[while(!std::mem::eof())];
};

CDFnd cdfnd @ 0x00;