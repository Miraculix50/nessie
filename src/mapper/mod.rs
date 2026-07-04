use std::cell::RefCell;
use std::rc::Rc;

use crate::cartridge::Mirroring;
use crate::cartridge::Rom;

pub mod nrom;

use crate::mapper::nrom::Nrom;

/// Mapper trait — every cartridge mapper implements this.
///
/// # Contract
///
/// - `read_prg` / `write_prg`: CPU address space `0x8000–0xFFFF` (plus `0x6000–0x7FFF` RAM).
/// - `read_chr` / `write_chr`: PPU address space `0x0000–0x1FFF` (CHR ROM / CHR RAM).
/// - `mirroring`: current nametable mirroring mode.
pub trait Mapper {
    fn read_prg(&self, addr: u16) -> u8;
    fn write_prg(&mut self, addr: u16, data: u8);
    fn read_chr(&self, addr: u16) -> u8;
    fn write_chr(&mut self, addr: u16, data: u8);
    fn mirroring(&self) -> Mirroring;
}

pub fn create_mapper(rom: Rom) -> Rc<RefCell<dyn Mapper>> {
    match rom.mapper {
        0 => Rc::new(RefCell::new(Nrom::new(rom))),
        n => unimplemented!("Mapper {} is not supported yet!", n),
    }
}

#[cfg(test)]
mod tests {
    use super::Mapper;
    use super::create_mapper;
    use crate::cartridge::{Mirroring, Rom};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_create_mapper_0_returns_some_mapper() {
        let mapper = create_mapper(crate::cartridge::test::test_rom(vec![0; 32]));
        // Type must be Rc<RefCell<dyn Mapper>>
        let _rc: Rc<RefCell<dyn Mapper>> = mapper;
    }

    #[test]
    #[should_panic(expected = "not supported")]
    fn test_create_mapper_unsupported_panics() {
        let rom = Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 2,
            screen_mirroring: Mirroring::Horizontal,
        };
        create_mapper(rom);
    }

    #[test]
    fn test_mapper_read_prg_returns_u8() {
        let mapper = create_mapper(crate::cartridge::test::test_rom(vec![0; 32]));
        let m = mapper.borrow();
        let _val: u8 = m.read_prg(0x8000);
    }

    #[test]
    fn test_mapper_write_prg_accepts_write() {
        let mapper = create_mapper(crate::cartridge::test::test_rom(vec![0; 32]));
        mapper.borrow_mut().write_prg(0x8000, 0x42);
    }

    #[test]
    fn test_mapper_read_chr_returns_u8() {
        let mapper = create_mapper(crate::cartridge::test::test_rom(vec![0; 32]));
        let m = mapper.borrow();
        let _val: u8 = m.read_chr(0x0000);
    }

    #[test]
    fn test_mapper_write_chr_accepts_write() {
        let mapper = create_mapper(crate::cartridge::test::test_rom(vec![0; 32]));
        mapper.borrow_mut().write_chr(0x0000, 0x42);
    }

    #[test]
    fn test_mapper_mirroring_returns_mirroring() {
        let rom = Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Vertical,
        };
        let mapper = create_mapper(rom);
        assert_eq!(mapper.borrow().mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn test_mapper_mirroring_horizontal() {
        let rom = Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        };
        let mapper = create_mapper(rom);
        assert_eq!(mapper.borrow().mirroring(), Mirroring::Horizontal);
    }

    #[test]
    fn test_create_mapper_from_test_rom() {
        let mapper = create_mapper(crate::cartridge::test::test_rom(vec![0x42; 32]));
        let m = mapper.borrow();
        assert_eq!(m.read_prg(0x8000), 0x42);
        // 0xC000 → prg[0x4000] = 0 (only first 32 bytes are 0x42)
        assert_eq!(m.read_prg(0xC000), 0x00);
    }
}
