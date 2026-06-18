use crate::cartridge::Mirroring;
use registers::addr::AddrRegister;

pub mod registers;

pub struct PPU {
    pub chr_rom: Vec<u8>,
    pub palette_table: [u8; 32],
    pub vram: [u8; 2048],
    pub oam_data: [u8; 256],

    pub mirroring: Mirroring,

    addr: AddrRegister,
}

impl PPU {
    fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        PPU {
            chr_rom: chr_rom,
            mirroring: mirroring,
            vram: [0; 2048],
            oam_data: [0; 64 * 4],
            palette_table: [0; 32],
            addr: AddrRegister::new(),
        }
    }

    fn write_to_ppu_addr(&mut self, value: u8) {
        self.addr.update(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ppu_addr_assembles_16bit_via_two_writes() {
        let mut addr = AddrRegister::new();
        addr.update(0x23);
        addr.update(0x05);
        assert_eq!(addr.get(), 0x2305);
    }

    #[test]
    fn test_ppu_addr_latch_toggles() {
        let mut addr = AddrRegister::new();
        addr.update(0x23);
        addr.update(0x05);
        addr.update(0x02);
        // After 3 writes: hi (0x23), lo (0x05), hi (0x02) — low byte unchanged
        assert_eq!(addr.get(), 0x0205);
    }

    #[test]
    fn test_ppu_addr_increment_carries() {
        let mut addr = AddrRegister::new();
        addr.update(0x23);
        addr.update(0xFF);
        addr.increment(1);
        assert_eq!(addr.get(), 0x2400);
    }

    #[test]
    fn test_ppu_addr_mirrors_above_0x3fff() {
        let mut addr = AddrRegister::new();
        addr.update(0x40);
        addr.update(0x00);
        assert_eq!(addr.get(), 0x0000);
    }

    #[test]
    fn test_ppu_addr_reset_latch() {
        let mut addr = AddrRegister::new();
        addr.update(0x23);
        addr.reset_latch();
        addr.update(0x02);
        // Reset forced hi_ptr back to true, so this write goes to high byte
        assert_eq!(addr.get(), 0x0200);
    }
}
