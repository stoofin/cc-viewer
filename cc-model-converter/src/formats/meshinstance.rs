use binread::{BinRead};
use crate::CurPos;

#[derive(BinRead)]
pub struct Command {
    startPos: CurPos,
    pub commandCode: u8,
    pub lengthTimesTwo: u8,

    #[br(args(commandCode, lengthTimesTwo))]
    pub command: MeshCommand,

    endPos: CurPos,
    #[br(assert(startPos.0 + lengthTimesTwo as u64 / 2 == endPos.0))]
    empty: (),
    // #[br(count = lengthTimesTwo / 2 - 2)]
    // pub args: Vec<u8>,
}

pub struct CommandList {
    pub commands: Vec<Command>,
}

impl BinRead for CommandList {
    type Args = ();

    fn read_options<R: std::io::Read + std::io::Seek>(reader: &mut R, _options: &binread::ReadOptions, _args: Self::Args) -> binread::BinResult<Self> {
        let mut commands = vec![];
        loop {
            let command = Command::read(reader)?;
            let isDone = command.commandCode == 0;
            commands.push(command);
            if isDone {
                return Ok(CommandList { commands });
            }
        }
    }
}

#[derive(BinRead)]
pub struct MeshInstance {
    pub zero1: u32,
    pub zero2: u32,
    pub counter: u16,
    pub spawnInterval: u16, // paired with 0x0f ttl command
    pub two: u32, // runtime time til next spawn, starts at spawnInterval ticks down to 0
    pub maybeStart: u32,
    pub zero3: u32,

    pub startCommand: u16,
    #[br(assert(startCommand == 0x0401, "Invalid start command"))]

    pub commandList: CommandList,

    pub curPos: CurPos,
    #[br(count = (4 - curPos.0 % 4) % 4)]
    pub endPadding: Vec<u8>,
    empty: (),
}


#[derive(BinRead)]
pub struct SinusoidalComponent {
    pub amplitude: i16,
    pub speed: i16,
    pub unknown1: u16,
    pub unknown2: u16,
}
#[derive(BinRead)]
pub struct SinusoidalAnimation {
    pub x: SinusoidalComponent,
    pub y: SinusoidalComponent,
    pub z: SinusoidalComponent
}

#[derive(BinRead)]
#[br(import(command: u8, _lengthTimesTwo: u8))]
pub enum MeshCommand {
    #[br(pre_assert(command == 0x00))] End,
    #[br(pre_assert(command == 0x05))] Mesh {
        unknown1: u16,
        unknown2: u16,
        unknown3: u16,
        name: [u8; 4],
        refType: u8,
        unknown4: [u8; 3],
    },
    #[br(pre_assert(command == 0x06))] BlendMode {
        unknown1: [u8; 2],
        // Not 100% sure on this command, vaguely confident in opaque blend and add, but subtract and quarter add need to be tested
        // battle/effects/tech/hensin.prd:sdr1 and sdr0 use tmuz with 3 blend mode
        // a lot of the battle/effects/black use 3 as subtract and it looks right
        // No idea what the other 5 bytes in the arguments do, and the first and last two are usually not zero.
        blend_mode: u8, // 0 => opaque, 1 => blend, 2 => add, presumably 3 => subtract, 4 => quarter add
        unknown2: [u8; 3],
    },
    #[br(pre_assert(command == 0x08))] Translation {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x0a))] Velocity {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x0c))] Acceleration {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x0f))] Ttl {
        ttl: u16,
    },
    #[br(pre_assert(command == 0x12))] Scale {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x14))] SinusoidalTranslation {
        animation: SinusoidalAnimation,
    },
    #[br(pre_assert(command == 0x15))] ScaleVelocity {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x19))] Rotation {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x1b))] RotationVelocity {
        x: i16,
        y: i16,
        z: i16,
    },
    #[br(pre_assert(command == 0x20))] Color {
        r: u16,
        g: u16,
        b: u16,
    },
    #[br(pre_assert(command == 0x21))] SinusoidalColor {
        animation: SinusoidalAnimation,
    },
    #[br(pre_assert(command == 0x22))] ColorVelocity {
        r: i16,
        g: i16,
        b: i16,
    },

    // 0x35 has something to do with flipping faces (accompanies negative scale), but not sure exactly what
    
    
    #[br(pre_assert(true))] Unknown {
        #[br(calc(command))]
        command: u8,
        #[br(count = _lengthTimesTwo / 2 - 2)]
        args: Vec<u8>,
    },
}