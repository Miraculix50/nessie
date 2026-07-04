use std::cell::RefCell;
use std::rc::Rc;

use crate::cartridge::Mirroring;
use crate::mapper::Mapper;
use crate::render::frame::Frame;
use crate::render::palette;
use registers::addr::AddrRegister;
use registers::control::ControlRegister;
use registers::mask::MaskRegister;
use registers::scroll::ScrollRegister;
use registers::status::StatusRegister;

pub mod registers;

pub struct PPU {
    pub chr_mapper: Rc<RefCell<dyn Mapper>>,
    pub vram: [u8; 2048],

    pub ctrl: ControlRegister,
    pub mask: MaskRegister,
    pub status: StatusRegister,
    pub scroll: ScrollRegister,
    pub addr: AddrRegister,

    pub oam_addr: u8,
    pub oam_data: [u8; 256],
    pub palette_table: [u8; 32],

    internal_data_buf: u8,
    pub(crate) pending_scroll_x: Option<u8>,
    pub(crate) pending_scroll_y: Option<u8>,

    pub(crate) scanline: u16,
    pub(crate) cycles: u16,
    pub frame: Frame,
    pub nmi: bool,
}

impl PPU {
    pub fn new(mapper: Rc<RefCell<dyn Mapper>>) -> Self {
        PPU {
            chr_mapper: mapper,
            vram: [0; 2048],

            ctrl: ControlRegister::new(),
            mask: MaskRegister::new(),
            status: StatusRegister::new(),
            scroll: ScrollRegister::new(),
            addr: AddrRegister::new(),

            oam_addr: 0,
            oam_data: [0; 64 * 4],
            palette_table: [0; 32],

            internal_data_buf: 0,
            pending_scroll_x: None,
            pending_scroll_y: None,

            scanline: 0,
            cycles: 0,
            frame: Frame::new(),
            nmi: false,
        }
    }

    fn palette_addr(addr: u16) -> usize {
        let mirrored_addr = addr & 0x001F;
        let addr = match mirrored_addr {
            0x10 | 0x14 | 0x18 | 0x1c => mirrored_addr - 0x10,
            _ => mirrored_addr,
        };
        addr as usize
    }

    pub fn write_to_ppu_addr(&mut self, value: u8) {
        self.addr.update(value);
    }

    pub fn write_to_ctrl(&mut self, value: u8) {
        let before_nmi_status = self.ctrl.generate_vblank_nmi();
        self.ctrl.update(value);
        if !before_nmi_status && self.ctrl.generate_vblank_nmi() && self.status.is_in_vblank() {
            self.nmi = true;
        }
    }

    pub fn write_to_mask(&mut self, value: u8) {
        self.mask.update(value);
    }

    pub fn write_to_scroll(&mut self, value: u8) {
        if self.scanline < 240 && self.cycles < 256 {
            if self.scroll.x_ptr {
                self.pending_scroll_x = Some(value)
            } else {
                self.pending_scroll_y = Some(value)
            }

            self.scroll.x_ptr = !self.scroll.x_ptr
        } else {
            self.scroll.write(value);
        }
    }

    pub fn write_to_oam_addr(&mut self, value: u8) {
        self.oam_addr = value;
    }

    pub fn write_to_oam_data(&mut self, value: u8) {
        self.oam_data[self.oam_addr as usize] = value;
        self.oam_addr = self.oam_addr.wrapping_add(1);
    }

    pub fn read_oam_data(&self) -> u8 {
        self.oam_data[self.oam_addr as usize]
    }

    pub fn write_oam_dma(&mut self, data: &[u8; 256]) {
        for x in data.iter() {
            self.oam_data[self.oam_addr as usize] = *x;
            self.oam_addr = self.oam_addr.wrapping_add(1);
        }
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
        match (&self.chr_mapper.borrow().mirroring(), name_table) {
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
                self.internal_data_buf = self.chr_mapper.borrow().read_chr(addr);
                result
            }
            0x2000..=0x2fff => {
                let result = self.internal_data_buf;
                self.internal_data_buf = self.vram[self.mirror_vram_addr(addr) as usize];
                result
            }
            0x3000..=0x3eff => {
                // unimplemented!(
                //     "Address space 0x3000..0x3eff is not expected to be used, requested = {}",
                //     addr
                // )
                0
            }
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
            0x3000..=0x3eff => {
                // unimplemented!(
                //     "Address space 0x3000..0x3eff is not expected to be used, requested = {}",
                //     addr
                // )
            }
            0x3f00..=0x3fff => self.palette_table[Self::palette_addr(addr)] = value,
            _ => panic!("Unexpected access to mirrored space {}", addr),
        }
        self.increment_vram_addr();
    }

    pub fn poll_nmi_interrupt(&mut self) -> bool {
        let nmi_status = self.nmi;
        self.nmi = false;
        nmi_status
    }

    /// Tick the PPU (cycle-accurate) the given amount of cycles, rendering one pixel at a time
    pub fn tick(&mut self, ppu_cycles: u16) -> bool {
        for _ in 0..ppu_cycles {
            if self.cycles == 0 {
                if let Some(x) = self.pending_scroll_x.take() {
                    self.scroll.set_scroll_x(x);
                }
                if let Some(y) = self.pending_scroll_y.take() {
                    self.scroll.set_scroll_y(y);
                }
            }

            if self.scanline < 240 && self.cycles < 256 {
                let pixel = self.render_pixel();
                let rgb = palette::SYSTEM_PALLETE[pixel as usize];
                self.frame
                    .set_pixel(self.cycles as usize, self.scanline as usize, rgb);
            }

            self.cycles += 1;
            if self.cycles == 341 {
                self.cycles = 0;
                self.scanline += 1;
                // VBlank phase
                if self.scanline == 241 {
                    self.status.set_vblank_status(true);
                    self.status.set_sprite_zero_hit(false);
                    if self.ctrl.generate_vblank_nmi() {
                        self.nmi = true;
                    }
                }
                // Frame end
                if self.scanline >= 262 {
                    self.scanline = 0;
                    self.nmi = false;
                    self.status.set_sprite_zero_hit(false);
                    self.status.reset_vblank_status();
                    return true;
                }
            }
        }

        return false;
    }

    fn render_pixel(&mut self) -> u8 {
        let bg = self.fetch_bg_pixel();

        // Sprite zero hit check
        if self.is_sprite_zero_hit(bg) {
            self.status.set_sprite_zero_hit(true);
        }

        // Draw sprite
        if let Some(s) = self.fetch_sprite_pixel() {
            if s != 0 {
                return s;
            }
        }

        bg // No sprite or transparent sprite -> bg
    }

    /// Fetches the sprite-pixel for the currently rendering pixel
    fn fetch_sprite_pixel(&self) -> Option<u8> {
        for i in (0..64).rev() {
            // find sprite (highest wins)
            let oam = &self.oam_data[(i * 4)..(i * 4 + 4)];
            let y = oam[0] as u16;
            let tile = oam[1] as u16;
            let attr = oam[2] as u16;
            let x = oam[3] as u16;

            let dy = self.scanline.wrapping_sub(y);
            let dx = self.cycles.wrapping_sub(x);
            if dy < 8 && dx < 8 {
                let mut fy = self.scanline - y;
                let mut fx = self.cycles - x;

                if attr & 0x80 != 0 {
                    // flip vertical
                    fy = 7 - fy
                }
                if attr & 0x40 != 0 {
                    // flip horizontal
                    fx = 7 - fx
                }

                let bank = self.ctrl.sprite_pattern_addr();
                let tile_addr = bank + tile * 16 + fy;
                let p0 = self.chr_mapper.borrow().read_chr(tile_addr);
                let p1 = self.chr_mapper.borrow().read_chr(tile_addr + 8);
                let value = ((p1 >> (7 - fx)) & 1) << 1 | ((p0 >> (7 - fx)) & 1);

                if value != 0 {
                    let palette_idx = attr & 0b11;
                    let start = (0x11 + palette_idx * 4) as usize;
                    let color = match value {
                        1 => self.palette_table[start],
                        2 => self.palette_table[start + 1],
                        3 => self.palette_table[start + 2],
                        _ => unreachable!(),
                    };
                    return Some(color);
                }
            }
        }

        None
    }

    /// Fetches the background-pixel for the currently rendering pixel
    fn fetch_bg_pixel(&self) -> u8 {
        // Pixel position
        let nx = self.scroll.scroll_x as u32 + self.cycles as u32; // absolute pixel position
        let ny = self.scroll.scroll_y as u32 + self.scanline as u32;
        let fx = (nx & 7) as u8; // pixel inside the tile
        let fy = (ny & 7) as u8;
        let tile_col = (nx / 8) as u16; // tile index (0, 1, 2, ...)
        let tile_row = (ny / 8) as u16;

        // nametable + tile id
        let nt_addr = self.resolve_tile_addr(tile_col, tile_row);
        let vram_idx = self.mirror_vram_addr(nt_addr);
        let tile_id = self.vram[vram_idx as usize];

        // attribute byte
        let nt_base = nt_addr & !0x3FF; // Remove last 10 bits
        let local_col = tile_col % 32;
        let local_row = tile_row % 30;
        let attr_tile_col = local_col / 4;
        let attr_tile_row = local_row / 4;
        let attr_offset = attr_tile_row * 8 + attr_tile_col;
        let attr_addr = nt_base + 0x3C0 + attr_offset;
        let attr_vram = self.mirror_vram_addr(attr_addr);
        let attr = self.vram[attr_vram as usize];

        // decode palette index from attribute byte
        let palette_idx = match ((local_col % 4) / 2, (local_row % 4) / 2) {
            (0, 0) => attr & 0b11,
            (1, 0) => (attr >> 2) & 0b11,
            (0, 1) => (attr >> 4) & 0b11,
            (1, 1) => (attr >> 6) & 0b11,
            (_, _) => unreachable!(),
        };

        // decode chr bitplane
        let bank = self.ctrl.background_pattern_addr();
        let chr_addr = bank + tile_id as u16 * 16 + fy as u16;
        let p0 = self.chr_mapper.borrow().read_chr(chr_addr);
        let p1 = self.chr_mapper.borrow().read_chr(chr_addr + 8);
        let pixel = ((p1 >> (7 - fx)) & 1) << 1 | ((p0 >> (7 - fx)) & 1);

        // palette lookup
        let start = 1 + (palette_idx as usize) * 4;
        match pixel {
            0 => self.palette_table[0],
            1 => self.palette_table[start],
            2 => self.palette_table[start + 1],
            3 => self.palette_table[start + 2],
            _ => unreachable!(),
        }
    }

    fn resolve_tile_addr(&self, tile_col: u16, tile_row: u16) -> u16 {
        let base = self.ctrl.nametable_addr();

        match self.chr_mapper.borrow().mirroring() {
            Mirroring::Horizontal => {
                let (top_nt, bottom_nt) = if base == 0x2000 || base == 0x2400 {
                    (0x2000, 0x2800)
                } else {
                    (0x2800, 0x2000)
                };

                let (nt, local_row) = if tile_row >= 30 {
                    (bottom_nt, tile_row - 30)
                } else {
                    (top_nt, tile_row)
                };

                let local_row = local_row % 30;

                nt + local_row * 32 + (tile_col % 32)
            }

            Mirroring::Vertical => {
                let (left_nt, right_nt) = if base == 0x2000 || base == 0x2800 {
                    (0x2000, 0x2400)
                } else {
                    (0x2400, 0x2000)
                };

                let (nt, local_col) = if tile_col >= 32 {
                    (right_nt, tile_col - 32)
                } else {
                    (left_nt, tile_col)
                };

                let local_col = local_col % 32;

                nt + (tile_row % 30) * 32 + local_col
            }

            Mirroring::FourScreen => unimplemented!(),
        }
    }

    /// Check if the current pixel is a sprite-zero-hit
    fn is_sprite_zero_hit(&self, bg: u8) -> bool {
        // No hit it if sprite or background rendering is disabled
        if !(self.mask.show_sprites() && self.mask.show_background()) {
            return false;
        }
        // No hit if already hit
        if self.status.is_sprite_zero_hit() {
            return false;
        }
        // No hit if background is transparent
        if bg == 0 {
            return false;
        };

        // Extract sprite zero information
        let (s0_y, s0_tile, s0_attr, s0_x) = (
            self.oam_data[0],
            self.oam_data[1],
            self.oam_data[2],
            self.oam_data[3],
        );

        // Check if current pixel is inside 8x8 sprite
        let dy = self.scanline.wrapping_sub(s0_y as u16);
        let dx = self.cycles.wrapping_sub(s0_x as u16);
        if dy >= 8 || dx >= 8 {
            return false;
        }

        let mut fy = dy as u8;
        let mut fx = dx as u8;
        if s0_attr & 0x80 != 0 {
            // flip vertical
            fy = 7 - fy;
        }
        if s0_attr & 0x40 != 0 {
            // flip horizontal
            fx = 7 - fx;
        }

        let bank = self.ctrl.sprite_pattern_addr();
        let addr = bank + s0_tile as u16 * 16 + fy as u16;
        let p0 = self.chr_mapper.borrow().read_chr(addr);
        let p1 = self.chr_mapper.borrow().read_chr(addr + 8);

        let pixel = ((p1 >> (7 - fx)) & 1) << 1 | ((p0 >> (7 - fx)) & 1);
        pixel != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Rom;
    use crate::mapper::{Mapper, create_mapper};
    use crate::render::frame::Frame;
    use crate::render::palette;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// PPU mit NROM (leerem CHR), für Tests ohne CHR-Daten
    fn test_ppu(mirroring: Mirroring) -> PPU {
        PPU::new(create_mapper(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: mirroring,
        }))
    }

    /// PPU mit NROM und gegebenem CHR-ROM
    fn ppu_with_chr(chr_data: Vec<u8>, mirroring: Mirroring) -> PPU {
        PPU::new(create_mapper(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr_data,
            mapper: 0,
            screen_mirroring: mirroring,
        }))
    }

    // ----- PPU: CHR via Mapper (neue API, Schritt 5) -----

    #[test]
    fn test_ppu_new_accepts_mapper() {
        let mapper = create_mapper(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        });
        let _ppu = PPU::new(mapper);
    }

    #[test]
    fn test_ppu_has_chr_mapper_field() {
        let mapper = create_mapper(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        });
        let ppu = PPU::new(mapper);
        let _field: &Rc<RefCell<dyn Mapper>> = &ppu.chr_mapper;
    }

    #[test]
    fn test_ppu_reads_chr_through_mapper() {
        let mut ppu = ppu_with_chr(vec![0x42; 0x2000], Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x00);
        ppu.write_to_ppu_addr(0x00);
        let _val = ppu.read_data();
    }

    #[test]
    fn test_ppu_fetches_bg_pixel_chr_through_mapper() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        let _pixel = ppu.fetch_bg_pixel();
    }

    #[test]
    fn test_ppu_fetches_sprite_pixel_chr_through_mapper() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[0x13] = 0x30;
        ppu.oam_data[0] = 0;
        ppu.oam_data[1] = 0;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 0;
        let _pixel = ppu.fetch_sprite_pixel();
    }

    #[test]
    fn test_ppu_mirroring_vertical_from_mapper() {
        let ppu = test_ppu(Mirroring::Vertical);
        let _m = ppu.chr_mapper.borrow().mirroring();
    }

    #[test]
    fn test_ppu_mirroring_horizontal_from_mapper() {
        let ppu = test_ppu(Mirroring::Horizontal);
        let _m = ppu.chr_mapper.borrow().mirroring();
    }

    #[test]
    fn test_ppu_tick_renders_pixel_through_mapper() {
        let mut chr = vec![0u8; 0x2000];
        chr[..16].copy_from_slice(&[0xFF; 16]);
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        ppu.tick(1);
        let _pixel = &ppu.frame.data[..3];
    }

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
        let mut chr = vec![0u8; 0x2000];
        chr[0x100] = 0x42;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x01);
        ppu.write_to_ppu_addr(0x00);
        // First read: returns old buffer (0), loads 0x42 into buffer
        assert_eq!(ppu.read_data(), 0x00);
        // Second read: returns 0x42
        assert_eq!(ppu.read_data(), 0x42);
    }

    #[test]
    fn test_ppu_read_data_vram_uses_internal_buffer() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
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
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.palette_table[0x00] = 0x2D; // 0x00 is the same as 0x10 due to mirroring
        ppu.write_to_ppu_addr(0x3F);
        ppu.write_to_ppu_addr(0x10);
        // Palette reads return immediately — no buffer, no dummy read
        assert_eq!(ppu.read_data(), 0x2D);
    }

    // ----- PPU tick (scanline/cycle timing) -----

    /// Helper: advance PPU by N scanlines (341 cycles each)
    fn advance_scanlines(ppu: &mut PPU, n: u16) {
        for _ in 0..n {
            ppu.tick(341);
        }
    }

    #[test]
    fn test_ppu_tick_accumulates_cycles() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.tick(100);
        assert_eq!(ppu.cycles, 100);
        assert_eq!(ppu.scanline, 0);
    }

    #[test]
    fn test_ppu_tick_advances_scanline() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.tick(341);
        assert_eq!(ppu.cycles, 0);
        assert_eq!(ppu.scanline, 1);
    }

    #[test]
    fn test_ppu_tick_multiple_scanlines() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        advance_scanlines(&mut ppu, 3);
        assert_eq!(ppu.scanline, 3);
    }

    #[test]
    fn test_ppu_tick_vblank_at_scanline_241() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        advance_scanlines(&mut ppu, 240);
        assert!(!ppu.status.is_in_vblank());
        ppu.tick(341);
        assert!(ppu.status.is_in_vblank());
    }

    #[test]
    fn test_ppu_tick_frame_end_resets() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        advance_scanlines(&mut ppu, 261);
        assert!(ppu.status.is_in_vblank());
        let frame_done = ppu.tick(341);
        assert!(frame_done);
        assert_eq!(ppu.scanline, 0);
        assert_eq!(ppu.cycles, 0);
        assert!(!ppu.status.is_in_vblank());
    }

    #[test]
    fn test_ppu_tick_nmi_triggered_when_enabled() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.ctrl.update(0b10000000);
        advance_scanlines(&mut ppu, 241);
        assert!(ppu.poll_nmi_interrupt());
    }

    #[test]
    fn test_ppu_tick_nmi_not_triggered_when_disabled() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        advance_scanlines(&mut ppu, 241);
        assert!(!ppu.poll_nmi_interrupt());
    }

    #[test]
    fn test_ppu_nmi_on_ctrl_write_during_vblank() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        advance_scanlines(&mut ppu, 241);
        assert!(ppu.status.is_in_vblank());
        ppu.write_to_ctrl(0b10000000);
        assert!(ppu.poll_nmi_interrupt());
    }

    // ----- write_data -----

    #[test]
    fn test_ppu_write_data_vram() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);
        ppu.write_data(0x66);
        assert_eq!(ppu.vram[0x0305], 0x66);
    }

    #[test]
    fn test_ppu_write_data_palette() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x3F);
        ppu.write_to_ppu_addr(0x10);
        ppu.write_data(0x2D);
        assert_eq!(ppu.palette_table[0x0], 0x2D); // 0x10 is internally mirrored to 0x00
    }

    #[test]
    fn test_ppu_write_data_increments_address() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x00);
        ppu.write_data(0x11);
        ppu.write_data(0x22);
        assert_eq!(ppu.vram[0x000], 0x11);
        assert_eq!(ppu.vram[0x001], 0x22);
    }

    // ----- OAM (Object Attribute Memory) -----

    #[test]
    fn test_oam_addr_write_sets_current_position() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.oam_addr, 0x10);
    }

    #[test]
    fn test_oam_write_read_round_trip() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.write_to_oam_addr(0x10);
        ppu.write_to_oam_data(0x66);
        ppu.write_to_oam_data(0x77);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_oam_data(), 0x66);

        ppu.write_to_oam_addr(0x11);
        assert_eq!(ppu.read_oam_data(), 0x77);
    }

    #[test]
    fn test_oam_addr_wraps_after_255() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.write_to_oam_addr(0xFF);
        ppu.write_to_oam_data(0xAA);
        assert_eq!(ppu.oam_addr, 0x00);
        ppu.write_to_oam_addr(0xFF);
        assert_eq!(ppu.read_oam_data(), 0xAA);
    }

    #[test]
    fn test_oam_dma_writes_256_bytes() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        let mut data = [0x66; 256];
        data[0] = 0x77;
        data[255] = 0x88;

        ppu.write_to_oam_addr(0x10);
        ppu.write_oam_dma(&data);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_oam_data(), 0x77);
        ppu.write_to_oam_addr(0x0F); // last byte (offset 0x10 + 255) wrapped to 0x0F
        assert_eq!(ppu.read_oam_data(), 0x88);
    }

    // ----- Phase 1: pixel rendering in tick() -----

    #[test]
    fn test_tick_renders_first_pixel() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        ppu.tick(1);
        let c = palette::SYSTEM_PALLETE[0x30];
        assert_eq!(ppu.frame.data[..3], [c.0, c.1, c.2]);
    }

    #[test]
    fn test_tick_renders_entire_first_scanline() {
        let mut chr = vec![0u8; 0x2000];
        chr[..16].copy_from_slice(&[0xFF; 16]);
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        for _ in 0..256 {
            ppu.tick(1);
        }
        let c = palette::SYSTEM_PALLETE[0x30];
        for x in 0..256 {
            let base = x * 3;
            assert_eq!(
                ppu.frame.data[base..base + 3],
                [c.0, c.1, c.2],
                "pixel ({}, 0) should be set",
                x
            );
        }
    }

    #[test]
    fn test_tick_does_not_render_during_hblank() {
        let mut chr = vec![0u8; 0x2000];
        chr[..16].copy_from_slice(&[0xFF; 16]);
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        // Tick past visible area (256) into hblank (256..340)
        for _ in 0..300 {
            ppu.tick(1);
        }
        // Pixel (256,0) should not be written (clamped to 255 max)
        // The test just verifies tick doesn't panic; pixel count is verified
        // by checking that only 256 pixels are non-zero in row 0
        let mut set_count = 0;
        for x in 0..341 {
            let base = x * 3;
            if ppu.frame.data[base..base + 3] != [0, 0, 0] {
                set_count += 1;
            }
        }
        assert_eq!(set_count, 256);
    }

    #[test]
    fn test_tick_does_not_render_during_vblank() {
        let mut chr = vec![0u8; 0x2000];
        chr[..16].copy_from_slice(&[0xFF; 16]);
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        // Advance to scanline 241 (vblank starts)
        advance_scanlines(&mut ppu, 241);
        // Snapshot frame before vblank ticks
        let before = ppu.frame.data.clone();
        // Tick during vblank — no pixels should be written
        for _ in 0..256 {
            ppu.tick(1);
        }
        // Frame data must be identical (no pixel written during vblank)
        assert_eq!(ppu.frame.data, before);
    }

    #[test]
    fn test_tick_renders_second_scanline_at_correct_y() {
        let mut chr = vec![0u8; 0x2000];
        chr[..16].copy_from_slice(&[0xFF; 16]);
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        // Complete first scanline, then render one pixel of second
        ppu.tick(341); // scanline 0 done
        ppu.tick(1); // first pixel of scanline 1
        let c = palette::SYSTEM_PALLETE[0x30];
        let base = 1 * Frame::WIDTH * 3; // row 1, pixel 0
        assert_eq!(
            ppu.frame.data[base..base + 3],
            [c.0, c.1, c.2],
            "pixel (0, 1) should be set on second scanline"
        );
    }

    #[test]
    fn test_frame_complete_has_rendered_data() {
        let mut chr = vec![0u8; 0x2000];
        chr[..16].copy_from_slice(&[0xFF; 16]);
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        for _ in 0..262 {
            ppu.tick(341);
        }
        // Check a few pixels across the frame
        let c = palette::SYSTEM_PALLETE[0x30];
        assert_eq!(ppu.frame.data[..3], [c.0, c.1, c.2]);
        let mid = 100 * Frame::WIDTH * 3;
        assert_eq!(ppu.frame.data[mid..mid + 3], [c.0, c.1, c.2]);
    }

    // ----- Phase 1: fetch_bg_pixel() -----

    #[test]
    fn test_fetch_bg_pixel_returns_correct_value() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        let pixel = ppu.fetch_bg_pixel();
        assert_eq!(pixel, 0x30);
    }

    #[test]
    fn test_fetch_bg_pixel_uses_correct_nametable_tile() {
        let mut chr = vec![0u8; 0x2000];
        chr[16] = 0b1000_0000;
        chr[24] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[3] = 0x30;
        // Place tile ID 1 at VRAM position 0
        ppu.vram[0] = 1;
        let pixel = ppu.fetch_bg_pixel();
        assert_eq!(pixel, 0x30);
    }

    // ----- Phase 1: fetch_sprite_pixel() -----

    #[test]
    fn test_fetch_sprite_pixel_returns_none_when_no_sprite() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        // All sprites off-screen (Y=255)
        ppu.oam_data = [0xFF; 256];
        let pixel = ppu.fetch_sprite_pixel();
        assert_eq!(pixel, None);
    }

    #[test]
    fn test_fetch_sprite_pixel_returns_color_for_visible_sprite() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[0x13] = 0x30; // sprite palette 0, color 3
        // Sprite 0 at (scanline=0, X=0), tile 0, palette 0
        ppu.oam_data[0] = 0; // Y
        ppu.oam_data[1] = 0; // tile index
        ppu.oam_data[2] = 0; // attributes (palette 0)
        ppu.oam_data[3] = 0; // X
        let pixel = ppu.fetch_sprite_pixel();
        assert_eq!(pixel, Some(0x30));
    }

    // ----- Phase 1: render_pixel() (BG/sprite compositing) -----

    #[test]
    fn test_render_pixel_sprite_over_bg() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        chr[16] = 0b1000_0000;
        chr[24] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[0x03] = 0x2D; // BG palette 0, color 3
        ppu.palette_table[0x13] = 0x15; // sprite palette 0, color 3
        // Sprite 0 at (0,0), tile 1, pixel value 3
        ppu.oam_data[0] = 0;
        ppu.oam_data[1] = 1;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 0;
        let pixel = ppu.render_pixel();
        // Sprite pixel (0x15) should win over BG pixel (0x2D)
        assert_eq!(pixel, 0x15);
    }

    #[test]
    fn test_render_pixel_transparent_sprite_shows_bg() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.palette_table[0x03] = 0x2D; // BG palette 0, color 3
        // No visible sprite (all off-screen)
        ppu.oam_data = [0xFF; 256];
        let pixel = ppu.render_pixel();
        // BG shows through
        assert_eq!(pixel, 0x2D);
    }

    #[test]
    fn test_render_pixel_transparent_everything() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        // Everything default: BG pixel=0, no visible sprite
        ppu.oam_data = [0xFF; 256];
        let pixel = ppu.render_pixel();
        assert_eq!(pixel, 0); // universal background
    }

    // ----- Sprite Zero Hit -----

    #[test]
    fn test_sprite_zero_hit_exact_coordinates() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.mask.update(0b0001_1000);
        ppu.scanline = 20;
        ppu.cycles = 10;
        ppu.oam_data[0] = 20; // Y
        ppu.oam_data[1] = 0; // tile
        ppu.oam_data[2] = 0; // attr
        ppu.oam_data[3] = 10; // X
        assert!(ppu.is_sprite_zero_hit(1));
    }

    #[test]
    fn test_sprite_zero_hit_requires_both_visible() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.scanline = 20;
        ppu.cycles = 10;
        ppu.oam_data[0] = 20;
        ppu.oam_data[1] = 0;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 10;
        // mask defaults to 0 = rendering disabled
        assert!(!ppu.is_sprite_zero_hit(1));
    }

    #[test]
    fn test_sprite_zero_hit_already_hit() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.mask.update(0b0001_1000);
        ppu.scanline = 20;
        ppu.cycles = 10;
        ppu.oam_data[0] = 20;
        ppu.oam_data[1] = 0;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 10;
        ppu.status.set_sprite_zero_hit(true);
        assert!(!ppu.is_sprite_zero_hit(1));
    }

    #[test]
    fn test_sprite_zero_hit_bg_transparent() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.mask.update(0b0001_1000);
        ppu.scanline = 20;
        ppu.cycles = 10;
        ppu.oam_data[0] = 20;
        ppu.oam_data[1] = 0;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 10;
        assert!(!ppu.is_sprite_zero_hit(0));
    }

    #[test]
    fn test_sprite_zero_hit_outside_sprite_area() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.mask.update(0b0001_1000);
        ppu.scanline = 50;
        ppu.cycles = 50;
        ppu.oam_data[0] = 20;
        ppu.oam_data[1] = 0;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 10;
        // dy=30 >= 8, dx=40 >= 8
        assert!(!ppu.is_sprite_zero_hit(1));
    }

    #[test]
    fn test_sprite_zero_hit_sprite_pixel_transparent() {
        let chr = vec![0u8; 0x2000]; // all transparent
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.mask.update(0b0001_1000);
        ppu.scanline = 20;
        ppu.cycles = 10;
        ppu.oam_data[0] = 20;
        ppu.oam_data[1] = 0;
        ppu.oam_data[2] = 0;
        ppu.oam_data[3] = 10;
        assert!(!ppu.is_sprite_zero_hit(1));
    }

    #[test]
    fn test_sprite_zero_hit_via_render_pixel_sets_status_flag() {
        let mut chr = vec![0u8; 0x2000];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let mut ppu = ppu_with_chr(chr, Mirroring::Horizontal);
        ppu.mask.update(0b0001_1000);
        ppu.palette_table[3] = 0x2D; // BG palette 0, color 3 (non-zero)
        ppu.scanline = 0;
        ppu.cycles = 0;
        ppu.oam_data[0] = 0; // Y
        ppu.oam_data[1] = 0; // tile 0
        ppu.oam_data[2] = 0; // attr
        ppu.oam_data[3] = 0; // X
        let _pixel = ppu.render_pixel();
        assert!(ppu.status.is_sprite_zero_hit());
    }

    // ----- Phase 2: Mid-Frame Scroll -----

    #[test]
    fn test_mid_frame_scroll_write_during_visible_is_pending() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.scanline = 100;
        ppu.cycles = 10;
        ppu.write_to_scroll(42);
        assert_eq!(ppu.pending_scroll_x, Some(42));
        assert!(ppu.pending_scroll_y.is_none());
        assert_eq!(ppu.scroll.scroll_x, 0);
        assert_eq!(ppu.scroll.scroll_y, 0);
    }

    #[test]
    fn test_mid_frame_scroll_write_during_vblank_is_immediate() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.scanline = 250;
        ppu.write_to_scroll(42);
        assert!(ppu.pending_scroll_x.is_none());
        assert!(ppu.pending_scroll_y.is_none());
        assert_eq!(ppu.scroll.scroll_x, 42);
    }

    #[test]
    fn test_pending_scroll_applied_on_next_scanline() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.scanline = 100;
        ppu.cycles = 100;
        ppu.write_to_scroll(77);
        assert_eq!(ppu.pending_scroll_x, Some(77));
        assert!(ppu.pending_scroll_y.is_none());
        // Advance to end of current scanline (341 - 100 = 241 more cycles)
        ppu.tick(241);
        assert_eq!(ppu.scanline, 101);
        assert_eq!(ppu.cycles, 0);
        ppu.tick(1);
        // pending scroll applied when cycles wrapped to 0
        assert_eq!(ppu.scroll.scroll_x, 77);
        assert!(ppu.pending_scroll_x.is_none());
        assert!(ppu.pending_scroll_y.is_none());
    }

    #[test]
    fn test_mid_frame_double_write_both_pending() {
        let mut ppu = test_ppu(Mirroring::Horizontal);
        ppu.scanline = 30;
        ppu.cycles = 100;
        // SMB1: mid-frame double write (scroll_x then scroll_y=0)
        ppu.write_to_scroll(42); // scroll_x write
        assert_eq!(ppu.pending_scroll_x, Some(42));
        assert!(ppu.pending_scroll_y.is_none());
        ppu.write_to_scroll(0); // scroll_y write
        assert_eq!(ppu.pending_scroll_x, Some(42));
        assert_eq!(ppu.pending_scroll_y, Some(0));
        // Advance to next scanline — both should be applied
        ppu.tick(241);
        assert_eq!(ppu.scanline, 31);
        assert_eq!(ppu.cycles, 0);
        ppu.tick(1);
        assert_eq!(ppu.scroll.scroll_x, 42);
        assert_eq!(ppu.scroll.scroll_y, 0);
        assert!(ppu.pending_scroll_x.is_none());
        assert!(ppu.pending_scroll_y.is_none());
    }
}
