pub mod frame;
pub mod palette;

use crate::ppu::PPU;
use frame::Frame;

pub fn render(ppu: &PPU, frame: &mut Frame) {
    let bank = ppu.ctrl.background_pattern_addr();

    // Background
    for i in 0..0x03c0 {
        let tile_n = ppu.vram[i] as u16;
        let tile_column = i % 32;
        let tile_row = i / 32;
        let start = (bank + tile_n * 16) as usize;
        let tile = &ppu.chr_rom[start..(start + 16)];
        let palette = bg_palette(ppu, tile_column, tile_row);

        for y in 0..8 {
            let mut upper = tile[y];
            let mut lower = tile[y + 8];

            for x in (0..8).rev() {
                let value = (1 & lower) << 1 | (1 & upper);
                upper = upper >> 1;
                lower = lower >> 1;

                let rgb = match value {
                    0 => palette::SYSTEM_PALLETE[ppu.palette_table[0] as usize],
                    1 => palette::SYSTEM_PALLETE[palette[1] as usize],
                    2 => palette::SYSTEM_PALLETE[palette[2] as usize],
                    3 => palette::SYSTEM_PALLETE[palette[3] as usize],
                    _ => unreachable!(),
                };

                frame.set_pixel(tile_column * 8 + x, tile_row * 8 + y, rgb);
            }
        }
    }

    // Sprites
    for i in (0..ppu.oam_data.len()).step_by(4).rev() {
        // 4 Bytes:
        // Byte 0: Tile Y position
        // Byte 1: Tile index
        // Byte 2: Tile attributes
        // Byte 3: Tile X position
        let tile_idx = ppu.oam_data[i + 1] as u16;
        let tile_x = ppu.oam_data[i + 3] as usize;
        let tile_y = ppu.oam_data[i] as usize;

        let flip_vertical = ppu.oam_data[i + 2] >> 7 & 1 == 1;
        let flip_horizontal = ppu.oam_data[i + 2] >> 6 & 1 == 1;
        let palette_idx = ppu.oam_data[i + 2] & 0b11;
        let sprite_palette = sprite_palette(ppu, palette_idx);

        let bank = ppu.ctrl.sprite_pattern_addr();

        let start = (bank + tile_idx as u16 * 16) as usize;
        let tile = &ppu.chr_rom[start..(start + 16)];

        for y in 0..8 {
            let mut upper = tile[y];
            let mut lower = tile[y + 8];

            for x in (0..8).rev() {
                let value = (1 & lower) << 1 | (1 & upper);
                upper >>= 1;
                lower >>= 1;
                let rgb = match value {
                    0 => continue, // transparent pixel
                    1 => palette::SYSTEM_PALLETE[sprite_palette[1] as usize],
                    2 => palette::SYSTEM_PALLETE[sprite_palette[2] as usize],
                    3 => palette::SYSTEM_PALLETE[sprite_palette[3] as usize],
                    _ => unreachable!(),
                };
                match (flip_horizontal, flip_vertical) {
                    (false, false) => frame.set_pixel(tile_x + x, tile_y + y, rgb),
                    (true, false) => frame.set_pixel(tile_x + 7 - x, tile_y + y, rgb),
                    (false, true) => frame.set_pixel(tile_x + x, tile_y + 7 - y, rgb),
                    (true, true) => frame.set_pixel(tile_x + 7 - x, tile_y + 7 - y, rgb),
                };
            }
        }
    }
}

fn sprite_palette(ppu: &PPU, palette_idx: u8) -> [u8; 4] {
    let start = 0x11 + (palette_idx * 4) as usize;
    [
        0,
        ppu.palette_table[start],
        ppu.palette_table[start + 1],
        ppu.palette_table[start + 2],
    ]
}

fn bg_palette(ppu: &PPU, tile_column: usize, tile_row: usize) -> [u8; 4] {
    let attr_table_idx = tile_row / 4 * 8 + tile_column / 4;
    let attr_byte = ppu.vram[0x03c0 + attr_table_idx]; // using first nametable for now

    let palette_idx = match (tile_column % 4 / 2, tile_row % 4 / 2) {
        (0, 0) => attr_byte & 0b11,
        (1, 0) => (attr_byte >> 2) & 0b11,
        (0, 1) => (attr_byte >> 4) & 0b11,
        (1, 1) => (attr_byte >> 6) & 0b11,
        (_, _) => unreachable!(),
    };

    let palette_start = 1 + palette_idx as usize * 4;

    [
        ppu.palette_table[0],
        ppu.palette_table[palette_start],
        ppu.palette_table[palette_start + 1],
        ppu.palette_table[palette_start + 2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Mirroring;

    fn make_ppu(chr_rom: Vec<u8>) -> PPU {
        PPU::new(chr_rom, Mirroring::Horizontal)
    }

    /// Hide all sprites off-screen (bottom) so they don't overwrite background tiles
    fn hide_sprites(ppu: &mut PPU) {
        for i in (0..ppu.oam_data.len()).step_by(4) {
            ppu.oam_data[i] = 0xFF; // Y = 255 (off-screen)
        }
    }

    #[test]
    fn test_render_all_zeros() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        hide_sprites(&mut ppu);
        ppu.palette_table[0] = 0x0F;
        let mut frame = Frame::new();
        render(&ppu, &mut frame);
        let bg = palette::SYSTEM_PALLETE[0x0F];
        for y in 0..8 {
            for x in 0..8 {
                let base = y * 3 * Frame::WIDTH + x * 3;
                assert_eq!(
                    frame.data[base..base + 3],
                    [bg.0, bg.1, bg.2],
                    "pixel ({}, {}) should be universal bg color",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_render_tile_fills_8x8_area() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        hide_sprites(&mut ppu);
        ppu.palette_table[3] = 0x30; // palette 0, color 3
        ppu.chr_rom[..16].copy_from_slice(&[0xFF; 16]);
        let mut frame = Frame::new();
        render(&ppu, &mut frame);
        let c = palette::SYSTEM_PALLETE[0x30];
        for y in 0..8 {
            for x in 0..8 {
                let base = y * 3 * Frame::WIDTH + x * 3;
                assert_eq!(
                    frame.data[base..base + 3],
                    [c.0, c.1, c.2],
                    "pixel ({}, {}) should be value-3 color",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_render_second_tile_row_y_offset() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        hide_sprites(&mut ppu);
        ppu.palette_table[3] = 0x30;
        ppu.vram[32] = 1;
        ppu.chr_rom[16..32].copy_from_slice(&[0xFF; 16]);
        let mut frame = Frame::new();
        render(&ppu, &mut frame);
        let c = palette::SYSTEM_PALLETE[0x30];
        let base = 8 * 3 * Frame::WIDTH;
        assert_eq!(
            frame.data[base..base + 3],
            [c.0, c.1, c.2],
            "pixel (0, 8) should be value-3 color from tile at vram[32]"
        );
    }

    #[test]
    fn test_render_bank_select() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        hide_sprites(&mut ppu);
        ppu.palette_table[3] = 0x30;
        ppu.ctrl.update(0b00010000);
        ppu.chr_rom[0x1000..0x1016].copy_from_slice(&[0xFF; 22]);
        let mut frame = Frame::new();
        render(&ppu, &mut frame);
        let c = palette::SYSTEM_PALLETE[0x30];
        let base = 0;
        assert_eq!(
            frame.data[base..base + 3],
            [c.0, c.1, c.2],
            "pixel (0, 0) should render from CHR ROM bank 1"
        );
    }

    // ----- bg_palette (Working with Colors) -----

    #[test]
    fn test_bg_palette_attr_byte_returns_four_palette_indices() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        // Attribute byte 0b11100100:
        //   bits 0-1 (top-left):     00 → 0
        //   bits 2-3 (top-right):    01 → 1
        //   bits 4-5 (bottom-left):  10 → 2
        //   bits 6-7 (bottom-right): 11 → 3
        ppu.vram[0x3C0] = 0b11100100;
        assert_eq!(bg_palette(&ppu, 0, 0)[1], ppu.palette_table[1]);
        assert_eq!(bg_palette(&ppu, 2, 0)[1], ppu.palette_table[5]);
        assert_eq!(bg_palette(&ppu, 0, 4)[1], ppu.palette_table[9]);
        assert_eq!(bg_palette(&ppu, 4, 4)[1], ppu.palette_table[13]);
    }

    #[test]
    fn test_bg_palette_returns_universal_bg_as_first_element() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        ppu.palette_table[0] = 0x0F;
        let pal = bg_palette(&ppu, 0, 0);
        assert_eq!(pal[0], ppu.palette_table[0]);
    }

    #[test]
    fn test_bg_palette_two_neighboring_tiles_same_meta_tile() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        ppu.vram[0x3C0] = 0b00000001;
        // Top-left meta-tile (0..2 col, 0..2 row): palette 1
        // Top-right meta-tile (2..4 col, 0..2 row): palette 0
        let top_left = bg_palette(&ppu, 0, 0);
        let top_right = bg_palette(&ppu, 2, 0);
        assert_eq!(top_left[1], ppu.palette_table[5]); // palette 1, color 1
        assert_eq!(top_right[1], ppu.palette_table[1]); // palette 0, color 1
    }

    // ----- sprite_palette (Working with Colors) -----

    #[test]
    fn test_sprite_palette_first_color_is_zero() {
        let ppu = make_ppu(vec![0; 0x2000]);
        let pal = sprite_palette(&ppu, 0);
        assert_eq!(pal[0], 0);
    }

    #[test]
    fn test_sprite_palette_index_maps_to_correct_offset() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        ppu.palette_table[0x11] = 0x15;
        ppu.palette_table[0x12] = 0x16;
        ppu.palette_table[0x13] = 0x17;
        let pal = sprite_palette(&ppu, 0);
        assert_eq!(pal[1], 0x15);
        assert_eq!(pal[2], 0x16);
        assert_eq!(pal[3], 0x17);
    }

    #[test]
    fn test_sprite_palette_index_1() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        ppu.palette_table[0x15] = 0x2A;
        let pal = sprite_palette(&ppu, 1);
        assert_eq!(pal[1], 0x2A);
    }

    // ----- render with real palette colors -----

    #[test]
    fn test_render_uses_bg_palette_for_tile_colors() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        hide_sprites(&mut ppu);
        ppu.palette_table[0] = 0x01; // universal bg (value 0)
        ppu.palette_table[1] = 0x15; // palette 0, color 1
        // Correct NES plane order: tile[y] = plane0 (low bit), tile[y+8] = plane1 (high bit)
        // pixel value = (plane1 << 1) | plane0
        // Need pixel (0,0) = value 1 → plane0_bit7=1, plane1_bit7=0
        ppu.chr_rom[0] = 0b1000_0000; // plane 0 (low bit)
        ppu.chr_rom[8] = 0b0000_0000; // plane 1 (high bit)
        let mut frame = Frame::new();
        render(&ppu, &mut frame);
        let rgb = palette::SYSTEM_PALLETE[0x15];
        assert_eq!(frame.data[0..3], [rgb.0, rgb.1, rgb.2],
            "pixel (0,0) should use palette_table[1] via bg_palette");
    }

    #[test]
    fn test_render_tile_with_mixed_pixel_values() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        hide_sprites(&mut ppu);
        ppu.palette_table[0] = 0x01; // universal bg
        ppu.palette_table[1] = 0x23; // palette 0, color 1
        ppu.palette_table[2] = 0x27; // palette 0, color 2
        ppu.palette_table[3] = 0x30; // palette 0, color 3
        // Row 0: bits select pixels 3, 2, 1, 0 (2-bit per pixel)
        // Correct NES plane order: tile[y] = plane0 (low bit), tile[y+8] = plane1 (high bit)
        // pixel value = (plane1 << 1) | plane0
        // Column 0 (bit 7): value=3 → plane0=1, plane1=1 → both bit7=1
        // Column 1 (bit 6): value=2 → plane0=0, plane1=1
        // Column 2 (bit 5): value=1 → plane0=1, plane1=0
        // Column 3 (bit 4): value=0 → plane0=0, plane1=0
        ppu.chr_rom[0] = 0b1010_0000; // plane 0 (low bit)
        ppu.chr_rom[8] = 0b1100_0000; // plane 1 (high bit)
        let mut frame = Frame::new();
        render(&ppu, &mut frame);

        let rgb_0 = palette::SYSTEM_PALLETE[0x01];
        let rgb_1 = palette::SYSTEM_PALLETE[0x23];
        let rgb_2 = palette::SYSTEM_PALLETE[0x27];
        let rgb_3 = palette::SYSTEM_PALLETE[0x30];

        // Row 0, first 4 pixels: value 3, 2, 1, 0
        assert_eq!(frame.data[0..3], [rgb_3.0, rgb_3.1, rgb_3.2]);
        assert_eq!(frame.data[3..6], [rgb_2.0, rgb_2.1, rgb_2.2]);
        assert_eq!(frame.data[6..9], [rgb_1.0, rgb_1.1, rgb_1.2]);
        assert_eq!(frame.data[9..12], [rgb_0.0, rgb_0.1, rgb_0.2]);
    }
}
