use crate::cartridge::Rom;
use crate::cpu::Mem;

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;
const PRG_ROM: u16 = 0x8000;
const PRG_ROM_END: u16 = 0xFFFF;

pub struct Bus {
    cpu_vram: [u8; 2048],
    rom: Rom,
}

impl Bus {
    pub fn new(rom: Rom) -> Self {
        Bus {
            cpu_vram: [0; 2048],
            rom: rom,
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= PRG_ROM;
        if self.rom.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            // Mirror ROM if needed
            addr = addr % 0x4000;
        }
        self.rom.prg_rom[addr as usize]
    }

    pub fn set_reset_vector(&mut self, addr: u16) {
        self.rom.prg_rom[0x7FFC] = (addr & 0xFF) as u8;
        self.rom.prg_rom[0x7FFD] = (addr >> 8) as u8;
    }

    pub fn write_prg_rom(&mut self, addr: u16, data: u8) {
        self.rom.prg_rom[(addr - 0x8000) as usize] = data;
    }
}

impl Mem for Bus {
    fn mem_read(&self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirrored_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirrored_addr as usize]
            }
            PPU_REGISTERS..=PPU_REGISTERS_MIRRORS_END => {
                let _mirrored_addr = addr & 0b00100000_00000111;
                // todo!("PPU isn't supported yet!");
                0
            }
            PRG_ROM..=PRG_ROM_END => self.read_prg_rom(addr),
            _ => {
                println!("Ignoring mem access at 0x{:04x}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirrored_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirrored_addr as usize] = data;
            }
            PPU_REGISTERS..=PPU_REGISTERS_MIRRORS_END => {
                let _mirrored_addr = addr & 0b00100000_00000111;
                // todo!("PPU isn't supported yet!");
            }
            PRG_ROM..=PRG_ROM_END => {
                panic!("Attempt to write to cartridge ROM space!")
            }
            _ => {
                println!("Ignoring mem access at 0x{:04x}", addr);
            }
        }
    }
}
