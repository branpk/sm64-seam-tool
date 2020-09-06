use crate::{
    geo::{Point3f, Vector3f},
    process::Process,
};
use bytemuck::{cast_slice, from_bytes, Pod, Zeroable};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, mem::size_of};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub base_addresses: HashMap<String, usize>,
    pub game_versions: Vec<GameVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameVersion {
    pub name: String,
    pub globals: Globals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Globals {
    pub global_timer: u32,
    pub lakitu_state: u32,
    pub surfaces_allocated: u32,
    pub surface_pool: u32,
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

impl Surface {
    pub fn vertex1(&self) -> Point3f {
        Point3f::new(
            self.vertex1[0] as f32,
            self.vertex1[1] as f32,
            self.vertex1[2] as f32,
        )
    }

    pub fn vertex2(&self) -> Point3f {
        Point3f::new(
            self.vertex2[0] as f32,
            self.vertex2[1] as f32,
            self.vertex2[2] as f32,
        )
    }

    pub fn vertex3(&self) -> Point3f {
        Point3f::new(
            self.vertex3[0] as f32,
            self.vertex3[1] as f32,
            self.vertex3[2] as f32,
        )
    }

    pub fn vertices(&self) -> [Point3f; 3] {
        [self.vertex1(), self.vertex2(), self.vertex3()]
    }

    pub fn normal(&self) -> Vector3f {
        Vector3f::new(self.normal[0], self.normal[1], self.normal[2])
    }
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
