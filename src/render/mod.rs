pub mod frame;
pub mod palette;

use crate::ppu::PPU;
use frame::Frame;

pub fn render(ppu: &PPU, frame: &mut Frame) {
    let bank = ppu.ctrl.background_pattern_addr();

    for i in 0..0x03c0 {
        // first nametable for now
        let tile_n = ppu.vram[i] as u16;
        let tile_x = i % 32;
        let tile_y = i / 32;
        let start = (bank + tile_n * 16) as usize;
        let tile = &ppu.chr_rom[start..(start + 16)];

        for y in 0..8 {
            let mut upper = tile[y];
            let mut lower = tile[y + 8];

            for x in (0..8).rev() {
                let value = (1 & upper) << 1 | (1 & lower);
                upper = upper >> 1;
                lower = lower >> 1;

                let rgb = match value {
                    0 => palette::SYSTEM_PALLETE[0x01],
                    1 => palette::SYSTEM_PALLETE[0x23],
                    2 => palette::SYSTEM_PALLETE[0x27],
                    3 => palette::SYSTEM_PALLETE[0x30],
                    _ => unreachable!(),
                };

                frame.set_pixel(tile_x * 8 + x, tile_y * 8 + y, rgb);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cartridge::Mirroring;

    fn make_ppu(chr_rom: Vec<u8>) -> PPU {
        PPU::new(chr_rom, Mirroring::Horizontal)
    }

    #[test]
    fn test_render_all_zeros() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        let mut frame = Frame::new();
        render(&ppu, &mut frame);
        let bg = palette::SYSTEM_PALLETE[0x01];
        for y in 0..8 {
            for x in 0..8 {
                let base = y * 3 * Frame::WIDTH + x * 3;
                assert_eq!(
                    frame.data[base..base + 3],
                    [bg.0, bg.1, bg.2],
                    "pixel ({}, {}) should be background color",
                    x,
                    y
                );
            }
        }
    }

    #[test]
    fn test_render_tile_fills_8x8_area() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
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

    #[test]
    fn test_render_tile_with_mixed_pixel_values() {
        let mut ppu = make_ppu(vec![0; 0x2000]);
        // Row 0: bits select pixels 3, 2, 1, 0 (2-bit per pixel)
        // upper byte bit pattern shifted right each iteration
        ppu.chr_rom[0] = 0b1100_0000;
        ppu.chr_rom[8] = 0b1010_0000;
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
