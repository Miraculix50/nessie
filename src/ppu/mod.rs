use crate::cartridge::Mirroring;
use crate::ppu::registers::addr;
use registers::addr::AddrRegister;
use registers::control::ControlRegister;
use registers::mask::MaskRegister;
use registers::scroll::ScrollRegister;
use registers::status::StatusRegister;

pub mod registers;

pub struct PPU {
    pub chr_rom: Vec<u8>,
    pub palette_table: [u8; 32],
    pub vram: [u8; 2048],
    pub oam_data: [u8; 256],

    pub mirroring: Mirroring,

    pub ctrl: ControlRegister,
    pub mask: MaskRegister,
    pub status: StatusRegister,
    pub scroll: ScrollRegister,
    pub addr: AddrRegister,

    internal_data_buf: u8,
}

impl PPU {
    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        PPU {
            chr_rom: chr_rom,
            mirroring: mirroring,
            vram: [0; 2048],
            oam_data: [0; 64 * 4],
            palette_table: [0; 32],
            ctrl: ControlRegister::new(),
            mask: MaskRegister::new(),
            status: StatusRegister::new(),
            scroll: ScrollRegister::new(),
            addr: AddrRegister::new(),
            internal_data_buf: 0,
        }
    }

    fn palette_addr(addr: u16) -> usize {
        let addr = match addr {
            0x3f10 | 0x3f14 | 0x3f18 | 0x3fc => addr - 0x10,
            _ => addr,
        };
        (addr - 0x3f00) as usize
    }

    pub fn write_to_ppu_addr(&mut self, value: u8) {
        self.addr.update(value);
    }

    pub fn write_to_ctrl(&mut self, value: u8) {
        self.ctrl.update(value);
    }

    pub fn write_to_mask(&mut self, value: u8) {
        self.mask.update(value);
    }

    pub fn write_to_scroll(&mut self, value: u8) {
        self.scroll.write(value);
    }

    pub fn read_status(&mut self) -> u8 {
        let snapshot = self.status.snapshot();
        self.status.reset_vblank_status();
        self.addr.reset_latch();
        self.scroll.reset_latch();
        snapshot
    }

    fn increment_vram_addr(&mut self) {
        self.addr.increment(self.ctrl.vram_addr_increment());
    }

    /// Horizontal:
    /// [ A ] [ a']
    /// [ B ] [ b']
    ///
    /// Vertical:
    /// [ A ] [ B ]
    /// [ a'] [ b']
    pub fn mirror_vram_addr(&self, addr: u16) -> u16 {
        let mirrored_vram = addr & 0b10111111111111; // mirror down 0x3000-0x3eff to 0x2000-0x2eff
        let vram_index = mirrored_vram - 0x2000; // to index of vram
        let name_table = vram_index / 0x400; // to name table index
        match (&self.mirroring, name_table) {
            (Mirroring::Vertical, 2) | (Mirroring::Vertical, 3) => vram_index - 0x800,
            (Mirroring::Horizontal, 2) => vram_index - 0x400,
            (Mirroring::Horizontal, 1) => vram_index - 0x400,
            (Mirroring::Horizontal, 3) => vram_index - 0x800,
            _ => vram_index,
        }
    }

    pub fn read_data(&mut self) -> u8 {
        let addr = self.addr.get();
        self.increment_vram_addr();

        match addr {
            0..=0x1fff => {
                let result = self.internal_data_buf;
                self.internal_data_buf = self.chr_rom[addr as usize];
                result
            }
            0x2000..=0x2fff => {
                let result = self.internal_data_buf;
                self.internal_data_buf = self.vram[self.mirror_vram_addr(addr) as usize];
                result
            }
            0x3000..=0x3eff => unimplemented!(
                "Address space 0x3000..0x3eff is not expected to be used, requested = {}",
                addr
            ),
            0x3f00..=0x3fff => self.palette_table[Self::palette_addr(addr)],
            _ => panic!("Unexpected access to mirrored space {}", addr),
        }
    }

    pub fn write_data(&mut self, value: u8) {
        let addr = self.addr.get();

        match addr {
            0..=0x1fff => println!("Attempt to write to chr rom space 0x{:04x}", addr),
            0x2000..=0x2fff => {
                self.vram[self.mirror_vram_addr(addr) as usize] = value;
            }
            0x3000..=0x3eff => unimplemented!(
                "Address space 0x3000..0x3eff is not expected to be used, requested = {}",
                addr
            ),
            0x3f00..=0x3fff => self.palette_table[Self::palette_addr(addr)] = value,
            _ => panic!("Unexpected access to mirrored space {}", addr),
        }
        self.increment_vram_addr();
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

    // ----- ControlRegister -----

    #[test]
    fn test_ctrl_new_defaults_to_zero() {
        let ctrl = ControlRegister::new();
        assert_eq!(ctrl.bits(), 0);
    }

    #[test]
    fn test_ctrl_vram_increment() {
        let mut ctrl = ControlRegister::new();
        ctrl.update(0);
        assert_eq!(ctrl.vram_addr_increment(), 1);

        ctrl.update(0b00000100);
        assert_eq!(ctrl.vram_addr_increment(), 32);
    }

    // ----- MaskRegister -----

    #[test]
    fn test_mask_new_defaults_to_zero() {
        let mask = MaskRegister::new();
        assert_eq!(mask.bits(), 0);
    }

    #[test]
    fn test_mask_show_background() {
        let mut mask = MaskRegister::new();
        mask.update(0b00001000);
        assert!(mask.show_background());
    }

    #[test]
    fn test_mask_show_sprites() {
        let mut mask = MaskRegister::new();
        mask.update(0b00010000);
        assert!(mask.show_sprites());
    }

    // ----- StatusRegister -----

    #[test]
    fn test_status_new_defaults_to_zero() {
        let status = StatusRegister::new();
        assert_eq!(status.bits(), 0);
    }

    #[test]
    fn test_status_set_and_reset_vblank() {
        let mut status = StatusRegister::new();
        status.set_vblank_status(true);
        assert!(status.is_in_vblank());
        status.reset_vblank_status();
        assert!(!status.is_in_vblank());
    }

    #[test]
    fn test_status_snapshot() {
        let mut status = StatusRegister::new();
        status.set_vblank_status(true);
        let snapshot = status.snapshot();
        assert!(snapshot & 0b10000000 != 0);
    }

    // ----- ScrollRegister -----

    #[test]
    fn test_scroll_new_defaults() {
        let scroll = ScrollRegister::new();
        assert_eq!(scroll.scroll_x, 0);
        assert_eq!(scroll.scroll_y, 0);
    }

    #[test]
    fn test_scroll_write_x_then_y() {
        let mut scroll = ScrollRegister::new();
        scroll.write(0x42);
        assert_eq!(scroll.scroll_x, 0x42);
        scroll.write(0x17);
        assert_eq!(scroll.scroll_y, 0x17);
    }

    #[test]
    fn test_scroll_reset_latch() {
        let mut scroll = ScrollRegister::new();
        scroll.write(0x42);
        scroll.write(0x17);
        scroll.reset_latch();
        scroll.write(0x88);
        assert_eq!(scroll.scroll_x, 0x88);
        assert_eq!(scroll.scroll_y, 0x17);
    }

    // ----- read_data -----

    #[test]
    fn test_ppu_read_data_chr_rom_uses_internal_buffer() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.chr_rom[0x100] = 0x42;
        ppu.write_to_ppu_addr(0x01);
        ppu.write_to_ppu_addr(0x00);
        // First read: returns old buffer (0), loads 0x42 into buffer
        assert_eq!(ppu.read_data(), 0x00);
        // Second read: returns 0x42
        assert_eq!(ppu.read_data(), 0x42);
    }

    #[test]
    fn test_ppu_read_data_vram_uses_internal_buffer() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.vram[0x0305] = 0x66;
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);
        // First read: returns old buffer (0), loads 0x66 into buffer
        assert_eq!(ppu.read_data(), 0x00);
        // Second read: returns 0x66
        assert_eq!(ppu.read_data(), 0x66);
    }

    #[test]
    fn test_ppu_read_data_palette_returns_directly() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.palette_table[0x00] = 0x2D; // 0x00 is the same as 0x10 due to mirroring
        ppu.write_to_ppu_addr(0x3F);
        ppu.write_to_ppu_addr(0x10);
        // Palette reads return immediately — no buffer, no dummy read
        assert_eq!(ppu.read_data(), 0x2D);
    }

    // ----- PPU tick (scanline/cycle timing) -----

    #[test]
    fn test_ppu_tick_accumulates_cycles() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.tick(100);
        assert_eq!(ppu.cycles, 100);
        assert_eq!(ppu.scanline, 0);
    }

    #[test]
    fn test_ppu_tick_advances_scanline() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.tick(341);
        assert_eq!(ppu.cycles, 0);
        assert_eq!(ppu.scanline, 1);
    }

    #[test]
    fn test_ppu_tick_multiple_scanlines() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.tick(341 * 3);
        assert_eq!(ppu.scanline, 3);
    }

    #[test]
    fn test_ppu_tick_vblank_at_scanline_241() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        // Skip to scanline 240
        ppu.tick(341 * 240);
        assert!(!ppu.status.is_in_vblank());
        // One more scanline → scanline 241
        ppu.tick(341);
        assert!(ppu.status.is_in_vblank());
    }

    #[test]
    fn test_ppu_tick_frame_end_resets() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        // Skip to scanline 261 (one before frame end)
        ppu.tick(341 * 261);
        assert!(ppu.status.is_in_vblank());
        // One more scanline → scanline 262 → reset
        let frame_done = ppu.tick(341);
        assert!(frame_done);
        assert_eq!(ppu.scanline, 0);
        assert_eq!(ppu.cycles, 0);
        assert!(!ppu.status.is_in_vblank());
    }

    #[test]
    fn test_ppu_tick_nmi_triggered_when_enabled() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.ctrl.update(0b10000000); // enable NMI
        // Skip to scanline 241
        ppu.tick(341 * 241);
        assert_eq!(ppu.poll_nmi_interrupt(), Some(1));
    }

    #[test]
    fn test_ppu_tick_nmi_not_triggered_when_disabled() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.ctrl.update(0); // NMI disabled
        // Skip to scanline 241
        ppu.tick(341 * 241);
        assert_eq!(ppu.poll_nmi_interrupt(), None);
    }

    #[test]
    fn test_ppu_nmi_on_ctrl_write_during_vblank() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        // Enter VBlank
        ppu.tick(341 * 241);
        assert!(ppu.status.is_in_vblank());
        // Enable NMI while already in VBlank → immediate NMI
        ppu.write_to_ctrl(0b10000000);
        assert_eq!(ppu.poll_nmi_interrupt(), Some(1));
    }

    // ----- write_data -----

    #[test]
    fn test_ppu_write_data_vram() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);
        ppu.write_data(0x66);
        assert_eq!(ppu.vram[0x0305], 0x66);
    }

    #[test]
    fn test_ppu_write_data_palette() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x3F);
        ppu.write_to_ppu_addr(0x10);
        ppu.write_data(0x2D);
        assert_eq!(ppu.palette_table[0x0], 0x2D); // 0x10 is internally mirrored to 0x00
    }

    #[test]
    fn test_ppu_write_data_increments_address() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x00);
        ppu.write_data(0x11);
        ppu.write_data(0x22);
        assert_eq!(ppu.vram[0x000], 0x11);
        assert_eq!(ppu.vram[0x001], 0x22);
    }
}
