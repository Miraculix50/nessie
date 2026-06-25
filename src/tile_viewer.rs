use crate::render::frame::Frame;
use crate::render::palette;

fn render_tile(
    chr_rom: &[u8],
    bank: usize,
    tile_n: usize,
    frame: &mut Frame,
    offset_x: usize,
    offset_y: usize,
) {
    let bank_offset = bank * 0x1000;
    let start = bank_offset + tile_n * 16;
    let tile = &chr_rom[start..start + 16];

    for y in 0..=7 {
        let mut upper = tile[y];
        let mut lower = tile[y + 8];
        for x in (0..=7).rev() {
            let value = (1 & upper) << 1 | (1 & lower);
            upper >>= 1; // Next bits for next iteration
            lower >>= 1;
            let rgb = match value {
                0 => palette::SYSTEM_PALLETE[0x01],
                1 => palette::SYSTEM_PALLETE[0x23],
                2 => palette::SYSTEM_PALLETE[0x27],
                3 => palette::SYSTEM_PALLETE[0x30],
                _ => unreachable!(),
            };
            frame.set_pixel(offset_x + x, offset_y + y, rgb);
        }
    }
}

pub fn show_tile(chr_rom: &[u8], bank: usize, tile_n: usize) -> Frame {
    let mut frame = Frame::new();
    render_tile(chr_rom, bank, tile_n, &mut frame, 0, 0);
    frame
}

pub fn show_tile_bank(chr_rom: &[u8], bank: usize) -> Frame {
    let mut frame = Frame::new();
    let mut offset_x = 0;
    let mut offset_y = 0;

    for tile_n in 0..256 {
        render_tile(chr_rom, bank, tile_n, &mut frame, offset_x, offset_y);
        if (tile_n + 1) % 20 == 0 {
            offset_y += 10;
            offset_x = 0;
        } else {
            offset_x += 10;
        }
    }

    frame
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_tile_all_zeros() {
        let chr_rom = vec![0u8; 16];
        let frame = show_tile(&chr_rom, 0, 0);
        let rgb = palette::SYSTEM_PALLETE[0x01];
        assert_eq!(frame.data[..3], [rgb.0, rgb.1, rgb.2]);
    }

    #[test]
    fn test_show_tile_all_ones() {
        let chr_rom = vec![0xFFu8; 16];
        let frame = show_tile(&chr_rom, 0, 0);
        let rgb = palette::SYSTEM_PALLETE[0x30];
        assert_eq!(frame.data[..3], [rgb.0, rgb.1, rgb.2]);
    }

    #[test]
    fn test_show_tile_first_pixel_value_3() {
        let mut chr = vec![0u8; 16];
        chr[0] = 0b1000_0000;
        chr[8] = 0b1000_0000;
        let frame = show_tile(&chr, 0, 0);
        let rgb = palette::SYSTEM_PALLETE[0x30];
        assert_eq!(frame.data[..3], [rgb.0, rgb.1, rgb.2]);
    }

    #[test]
    fn test_show_tile_bank_select() {
        let mut chr_rom = vec![0u8; 0x2000];
        chr_rom[0x1000] = 0b1000_0000;
        chr_rom[0x1008] = 0b1000_0000;
        let frame = show_tile(&chr_rom, 1, 0);
        let rgb = palette::SYSTEM_PALLETE[0x30];
        assert_eq!(frame.data[..3], [rgb.0, rgb.1, rgb.2]);
    }

    #[test]
    fn test_show_tile_different_pixel_values() {
        let mut chr = vec![0u8; 16];
        chr[0] = 0b1100_0000;
        chr[8] = 0b1010_0000;
        let frame = show_tile(&chr, 0, 0);
        let rgb_0 = palette::SYSTEM_PALLETE[0x01];
        let rgb_1 = palette::SYSTEM_PALLETE[0x23];
        let rgb_2 = palette::SYSTEM_PALLETE[0x27];
        let rgb_3 = palette::SYSTEM_PALLETE[0x30];
        assert_eq!(frame.data[..3], [rgb_3.0, rgb_3.1, rgb_3.2]);
        assert_eq!(frame.data[3..6], [rgb_2.0, rgb_2.1, rgb_2.2]);
        assert_eq!(frame.data[6..9], [rgb_1.0, rgb_1.1, rgb_1.2]);
        assert_eq!(frame.data[9..12], [rgb_0.0, rgb_0.1, rgb_0.2]);
    }
}
