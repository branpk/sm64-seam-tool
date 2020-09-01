use crate::process::Process;
use bytemuck::{cast_slice, from_bytes, Pod, Zeroable};
use std::mem::size_of;

#[derive(Debug, Clone)]
pub struct Globals {
    pub global_timer: u32,
    pub lakitu_state: u32,
    pub surfaces_allocated: u32,
    pub surface_pool: u32,
}

impl Globals {
    pub const US: Globals = Globals {
        global_timer: 0x8032d5d4,
        lakitu_state: 0x8033c698,
        surfaces_allocated: 0x80361170,
        surface_pool: 0x8038ee9c,
    };

    pub const JP: Globals = Globals {
        global_timer: 0x8032c694,
        lakitu_state: 0x8033c698,
        surfaces_allocated: 0x8035fe00,
        surface_pool: 0x8038ee9c,
    };
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Surface {
    pub flags: u8,
    pub vertex1: [i16; 3],
    pub vertex2: [i16; 3],
    pub vertex3: [i16; 3],
    pub normal: [f32; 3],
    pub origin_offset: f32,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub lakitu_pos: [f32; 3],
    pub lakitu_focus: [f32; 3],
    pub surfaces: Vec<Surface>,
}

impl GameState {
    pub fn read(globals: &Globals, process: &Process) -> Self {
        let num_surfaces: u32 = process.read(globals.surfaces_allocated);
        let surface_pool_addr: u32 = process.read(globals.surface_pool);

        let surfaces = if surface_pool_addr != 0 {
            let bytes = process.read_bytes(surface_pool_addr, num_surfaces as usize * 0x30);

            bytes
                .chunks(0x30)
                .map(|chunk| {
                    let read_s16 = |offset: usize| {
                        let offset = if offset % 4 == 0 {
                            offset + 2
                        } else {
                            offset - 2
                        };
                        *from_bytes::<i16>(&chunk[offset..offset + 2])
                    };
                    let read_f32 = |offset: usize| *from_bytes::<f32>(&chunk[offset..offset + 4]);
                    let read_s16_3 = |offset: usize| {
                        [
                            read_s16(offset + 0),
                            read_s16(offset + 2),
                            read_s16(offset + 4),
                        ]
                    };
                    let read_f32_3 = |offset: usize| {
                        [
                            read_f32(offset + 0),
                            read_f32(offset + 4),
                            read_f32(offset + 8),
                        ]
                    };

                    Surface {
                        flags: chunk[0x07],
                        vertex1: read_s16_3(0x0A),
                        vertex2: read_s16_3(0x10),
                        vertex3: read_s16_3(0x16),
                        normal: read_f32_3(0x1C),
                        origin_offset: read_f32(0x28),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        Self {
            lakitu_pos: process.read(globals.lakitu_state + 0x8C),
            lakitu_focus: process.read(globals.lakitu_state + 0x80),
            surfaces,
        }
    }
}
