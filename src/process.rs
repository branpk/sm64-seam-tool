use bytemuck::{Pod, from_bytes};
use read_process_memory::{ProcessHandle, copy_address};
use std::mem::size_of;

pub struct Process {
    handle: ProcessHandle,
    base_address: usize,
}

impl Process {
    pub fn attach(pid: u32, base_address: usize) -> Self {
        Self {
            handle: pid.try_into().unwrap(),
            base_address,
        }
    }

    pub fn read_bytes(&self, virtual_address: u32, size: usize) -> Vec<u8> {
        let address = self.base_address + (virtual_address as usize & 0x3FFFFFFF);
        copy_address(address, size, &self.handle).unwrap()
    }

    pub fn read<T: Pod>(&self, virtual_address: u32) -> T {
        assert!(virtual_address % 4 == 0);
        let bytes = self.read_bytes(virtual_address, size_of::<T>());
        *from_bytes(&bytes)
    }
}
