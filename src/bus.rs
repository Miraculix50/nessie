use crate::cartridge::Rom;
use crate::cpu::Mem;
use crate::ppu::PPU;

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;
const PRG_ROM: u16 = 0x8000;
const PRG_ROM_END: u16 = 0xFFFF;

pub struct Bus {
    cpu_vram: [u8; 2048],
    prg_rom: Vec<u8>,
    pub ppu: PPU,

    pub cycles: u64,
}

impl Bus {
    pub fn new(rom: Rom) -> Self {
        let ppu = PPU::new(rom.chr_rom, rom.screen_mirroring);

        Bus {
            cpu_vram: [0; 2048],
            prg_rom: rom.prg_rom,
            ppu: ppu,
            cycles: 0,
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= PRG_ROM;
        if self.prg_rom.len() == 0x4000 && addr >= 0x4000 {
            // Mirror ROM if needed
            addr = addr % 0x4000;
        }
        self.prg_rom[addr as usize]
    }

    pub fn set_reset_vector(&mut self, addr: u16) {
        self.prg_rom[0x7FFC] = (addr & 0xFF) as u8;
        self.prg_rom[0x7FFD] = (addr >> 8) as u8;
    }

    pub fn write_prg_rom(&mut self, addr: u16, data: u8) {
        self.prg_rom[(addr - 0x8000) as usize] = data;
    }

    pub fn tick(&mut self, cycles: u16) {
        self.cycles += cycles as u64;
        self.ppu.tick(cycles as u16 * 3);
    }

    pub fn poll_nmi_status(&mut self) -> bool {
        self.ppu.poll_nmi_interrupt()
    }
}

impl Mem for Bus {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirrored_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirrored_addr as usize]
            }
            0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
                panic!(
                    "Attempt to read from write-only PPU register with address 0x{:04x}",
                    addr
                )
            }
            0x2002 => self.ppu.read_status(),
            0x2007 => self.ppu.read_data(),
            0x2008..=PPU_REGISTERS_MIRRORS_END => {
                let mirrored_addr = addr & 0b00100000_00000111;
                self.mem_read(mirrored_addr)
            }
            PRG_ROM..=PRG_ROM_END => self.read_prg_rom(addr),
            _ => {
                // println!("Ignoring mem access at 0x{:04x}", addr);
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
            0x2000 => self.ppu.write_to_ctrl(data),
            0x2001 => self.ppu.write_to_mask(data),
            0x2002 => {
                panic!(
                    "Attempt to write to read-only PPU register with address 0x{:04x}",
                    addr
                )
            }
            0x2003 | 0x2004 => unimplemented!(
                "Attempt to write to PPU register with the address 0x{:04x}",
                addr
            ),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_addr(data),
            0x2007 => self.ppu.write_data(data),
            0x2008..=PPU_REGISTERS_MIRRORS_END => {
                let mirrored_addr = addr & 0b00100000_00000111;
                self.mem_write(mirrored_addr, data);
            }
            PRG_ROM..=PRG_ROM_END => {
                panic!("Attempt to write to cartridge ROM space!")
            }
            _ => {
                // println!("Ignoring mem access at 0x{:04x}", addr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::test::test_rom;

    #[test]
    fn test_bus_write_2000_sets_ctrl() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2000, 0b10000000);
        assert_eq!(bus.ppu.ctrl.bits(), 0b10000000);
    }

    #[test]
    fn test_bus_write_2001_sets_mask() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2001, 0b00010000);
        assert!(bus.ppu.mask.show_sprites());
    }

    #[test]
    fn test_bus_read_2002_returns_status_and_clears_vblank() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.ppu.status.set_vblank_status(true);
        let status = bus.mem_read(0x2002);
        assert!(status & 0b10000000 != 0);
        assert!(!bus.ppu.status.is_in_vblank());
    }

    #[test]
    fn test_bus_read_2002_resets_addr_latch() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2006, 0x23);
        bus.mem_read(0x2002);
        bus.mem_write(0x2006, 0x42);
        // 0x42 << 8 = 0x4200, mirrored to 14-bit → 0x0200
        assert_eq!(bus.ppu.addr.get(), 0x0200);
    }

    #[test]
    fn test_bus_read_2002_resets_scroll_latch() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2005, 0x42);
        bus.mem_read(0x2002);
        bus.mem_write(0x2005, 0x99);
        assert_eq!(bus.ppu.scroll.scroll_x, 0x99);
    }

    #[test]
    fn test_bus_write_2005_sets_scroll_x() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2005, 0x42);
        assert_eq!(bus.ppu.scroll.scroll_x, 0x42);
    }

    #[test]
    fn test_bus_write_2006_sets_addr() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2006, 0x23);
        bus.mem_write(0x2006, 0x05);
        assert_eq!(bus.ppu.addr.get(), 0x2305);
    }

    #[test]
    fn test_bus_write_2007_writes_vram() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2006, 0x20);
        bus.mem_write(0x2006, 0x00);
        bus.mem_write(0x2007, 0x66);
        assert_eq!(bus.ppu.vram[0x000], 0x66);
    }

    #[test]
    fn test_bus_write_ppu_mirror_routes_to_ctrl() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.mem_write(0x2008, 0b10000000);
        assert_eq!(bus.ppu.ctrl.bits(), 0b10000000);
    }

    // ----- Bus tick (cycle counting) -----

    #[test]
    fn test_bus_tick_accumulates_cycles() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.tick(5);
        bus.tick(3);
        assert_eq!(bus.cycles, 8);
    }

    #[test]
    fn test_bus_tick_propagates_to_ppu() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.tick(1);
        assert_eq!(bus.ppu.cycles, 3);
    }

    #[test]
    fn test_bus_poll_nmi_status_returns_true_when_pending() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        bus.ppu.ctrl.update(0b10000000);
        for _ in 0..241 {
            bus.ppu.tick(341);
        }
        assert!(bus.poll_nmi_status());
    }

    #[test]
    fn test_bus_poll_nmi_status_returns_false_when_no_nmi() {
        let mut bus = Bus::new(test_rom(vec![0; 32]));
        assert!(!bus.poll_nmi_status());
    }
}
