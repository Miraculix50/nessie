use std::cell::RefCell;
use std::rc::Rc;

use crate::cpu::Mem;
use crate::joypad::Joypad;
use crate::mapper::Mapper;
use crate::ppu::PPU;

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1FFF;
const PPU_REGISTERS_MIRRORS: u16 = 0x2008;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3FFF;
const APU: u16 = 0x4000;
const APU_END: u16 = 0x4015;
const JOYPAD1: u16 = 0x4016;
const JOYPAD2: u16 = 0x4017;
const PRG_ROM: u16 = 0x8000;
const PRG_ROM_END: u16 = 0xFFFF;

pub struct Bus {
    cpu_vram: [u8; 2048],
    pub mapper: Rc<RefCell<dyn Mapper>>,
    pub ppu: PPU,
    pub joypad1: Joypad,
    pub frame_ready: bool,
    pub cycles: u64,
}

impl Bus {
    pub fn new(mapper: Rc<RefCell<dyn Mapper>>) -> Self {
        let ppu_mapper = mapper.clone();
        let ppu = PPU::new(ppu_mapper);

        Bus {
            cpu_vram: [0; 2048],
            mapper,
            ppu,
            joypad1: Joypad::new(),
            frame_ready: false,
            cycles: 0,
        }
    }

    fn read_prg_rom(&self, addr: u16) -> u8 {
        self.mapper.borrow().read_prg(addr)
    }

    pub fn set_reset_vector(&mut self, addr: u16) {
        self.write_prg_rom(0xFFFC, (addr & 0xFF) as u8);
        self.write_prg_rom(0xFFFD, (addr >> 8) as u8);
    }

    pub fn write_prg_rom(&mut self, addr: u16, data: u8) {
        self.mapper.borrow_mut().write_prg(addr, data);
    }

    pub fn tick(&mut self, cycles: u16) {
        self.cycles += cycles as u64;
        let nmi_before = self.ppu.nmi;
        self.ppu.tick(cycles as u16 * 3);
        if !nmi_before && self.ppu.nmi {
            self.frame_ready = true;
        }
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
                // panic!(
                //     "Attempt to read from write-only PPU register with address 0x{:04x}",
                //     addr
                // )
                0
            }
            0x2002 => self.ppu.read_status(),
            0x2004 => self.ppu.read_oam_data(),
            0x2007 => self.ppu.read_data(),
            PPU_REGISTERS_MIRRORS..=PPU_REGISTERS_MIRRORS_END => {
                let mirrored_addr = addr & 0b00100000_00000111;
                self.mem_read(mirrored_addr)
            }
            APU..=APU_END => {
                // ignore APU
                0
            }
            JOYPAD1 => self.joypad1.read(),
            JOYPAD2 => {
                // ignore JOYPAD2
                0
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
            0x2002 => panic!("Attempt to write to PPU status register"),
            0x2003 => self.ppu.write_to_oam_addr(data),
            0x2004 => self.ppu.write_to_oam_data(data),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_addr(data),
            0x2007 => self.ppu.write_data(data),
            PPU_REGISTERS_MIRRORS..=PPU_REGISTERS_MIRRORS_END => {
                let mirrored_addr = addr & 0b00100000_00000111;
                self.mem_write(mirrored_addr, data);
            }
            0x4014 => {
                let mut buffer: [u8; 256] = [0; 256];
                let hi = (data as u16) << 8;
                for i in 0..256u16 {
                    buffer[i as usize] = self.mem_read(hi + i);
                }

                self.ppu.write_oam_dma(&buffer);
            }
            APU..=APU_END => {
                // ignore APU
            }
            JOYPAD1 => self.joypad1.write(data),
            JOYPAD2 => {
                // ignore JOYPAD2
            }
            PRG_ROM..=PRG_ROM_END => self.mapper.borrow_mut().write_prg(addr, data),
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
    use crate::mapper::create_mapper;

    #[test]
    fn test_bus_write_2000_sets_ctrl() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.mem_write(0x2000, 0b10000000);
        assert_eq!(bus.ppu.ctrl.bits(), 0b10000000);
    }

    #[test]
    fn test_bus_write_2001_sets_mask() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.mem_write(0x2001, 0b00010000);
        assert!(bus.ppu.mask.show_sprites());
    }

    #[test]
    fn test_bus_read_2002_returns_status_and_clears_vblank() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.ppu.status.set_vblank_status(true);
        let status = bus.mem_read(0x2002);
        assert!(status & 0b10000000 != 0);
        assert!(!bus.ppu.status.is_in_vblank());
    }

    #[test]
    fn test_bus_read_2002_resets_addr_latch() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.mem_write(0x2006, 0x23);
        bus.mem_read(0x2002);
        bus.mem_write(0x2006, 0x42);
        // 0x42 << 8 = 0x4200, mirrored to 14-bit → 0x0200
        assert_eq!(bus.ppu.addr.get(), 0x0200);
    }

    #[test]
    fn test_bus_read_2002_resets_scroll_latch() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.ppu.scanline = 250; // VBLANK so scroll is applied instantly
        bus.mem_write(0x2005, 0x42);
        bus.mem_read(0x2002);
        bus.mem_write(0x2005, 0x99);
        assert_eq!(bus.ppu.scroll.scroll_x, 0x99);
    }

    #[test]
    fn test_bus_write_2005_sets_scroll_x() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.ppu.scanline = 250; // VBLANK so scroll is applied instantly
        bus.mem_write(0x2005, 0x42);
        assert_eq!(bus.ppu.scroll.scroll_x, 0x42);
    }

    #[test]
    fn test_bus_write_2006_sets_addr() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.ppu.scanline = 250; // VBLANK so scroll is applied instantly
        bus.mem_write(0x2006, 0x23);
        bus.mem_write(0x2006, 0x05);
        assert_eq!(bus.ppu.addr.get(), 0x2305);
    }

    #[test]
    fn test_bus_write_2007_writes_vram() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.mem_write(0x2006, 0x20);
        bus.mem_write(0x2006, 0x00);
        bus.mem_write(0x2007, 0x66);
        assert_eq!(bus.ppu.vram[0x000], 0x66);
    }

    #[test]
    fn test_bus_write_ppu_mirror_routes_to_ctrl() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.mem_write(0x2008, 0b10000000);
        assert_eq!(bus.ppu.ctrl.bits(), 0b10000000);
    }

    // ----- Bus tick (cycle counting) -----

    #[test]
    fn test_bus_tick_accumulates_cycles() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.tick(5);
        bus.tick(3);
        assert_eq!(bus.cycles, 8);
    }

    #[test]
    fn test_bus_tick_propagates_to_ppu() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.tick(1);
        assert_eq!(bus.ppu.cycles, 3);
    }

    #[test]
    fn test_bus_poll_nmi_status_returns_true_when_pending() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.ppu.ctrl.update(0b10000000);
        for _ in 0..241 {
            bus.ppu.tick(341);
        }
        assert!(bus.poll_nmi_status());
    }

    #[test]
    fn test_bus_poll_nmi_status_returns_false_when_no_nmi() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        assert!(!bus.poll_nmi_status());
    }

    // ----- Phase 1: Mapper routing through Bus -----

    #[test]
    fn test_bus_reads_prg_rom_through_mapper() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0x42; 32])));
        assert_eq!(bus.mem_read(0x8000), 0x42);
    }

    #[test]
    fn test_bus_writes_prg_rom_through_mapper() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0x42; 32])));
        bus.mem_write(0x8000, 0xFF);
        // NROM write_prg writes through (needed for CPU::load / set_reset_vector)
        assert_eq!(bus.mem_read(0x8000), 0xFF);
    }

    #[test]
    fn test_bus_reads_prg_rom_offset_through_mapper() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0x42; 32])));
        // 0xC000 → prg[0x4000] = 0 (only first 32 bytes are 0x42)
        assert_eq!(bus.mem_read(0xC000), 0x00);
    }

    #[test]
    fn test_bus_nrom_read_chr_rom_through_ppu() {
        let mut bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        bus.mem_write(0x2006, 0x00);
        bus.mem_write(0x2006, 0x00);
        let _dummy = bus.mem_read(0x2007);
        assert_eq!(bus.mem_read(0x2007), 0x02);
    }

    #[test]
    fn test_bus_has_mapper_field() {
        let bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        let _mapper = &bus.mapper;
    }

    #[test]
    fn test_bus_mapper_type_is_rc_refcell() {
        use crate::mapper::Mapper;
        use std::cell::RefCell;
        use std::rc::Rc;

        let bus = Bus::new(create_mapper(test_rom(vec![0; 32])));
        let _typed: &Rc<RefCell<dyn Mapper>> = &bus.mapper;
    }
}
