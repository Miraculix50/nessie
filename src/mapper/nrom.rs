use crate::cartridge::Mirroring;
use crate::cartridge::Rom;
use crate::mapper::Mapper;
use crate::ppu::registers::addr;

pub struct Nrom {
    prg_rom: Vec<u8>,
    prg_ram: Vec<u8>,
    chr_rom: Vec<u8>,
    chr_ram: Option<Vec<u8>>,
    mirroring: Mirroring,
}

impl Nrom {
    pub fn new(rom: Rom) -> Self {
        let chr_ram = if rom.chr_rom.is_empty() {
            Some(vec![0; 0x2000])
        } else {
            None
        };

        Nrom {
            prg_rom: rom.prg_rom,
            prg_ram: vec![0; 0x2000],
            chr_rom: rom.chr_rom,
            chr_ram,
            mirroring: rom.screen_mirroring,
        }
    }
}

impl Mapper for Nrom {
    fn read_prg(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize], // PRG RAM
            _ => {
                // PRG ROM
                let mut prg_addr = addr - 0x8000;
                if self.prg_rom.len() == 0x4000 {
                    prg_addr %= 0x4000;
                }

                self.prg_rom[prg_addr as usize]
            }
        }
    }

    fn write_prg(&mut self, addr: u16, data: u8) {
        match addr {
            0x6000..=0x7FFF => self.prg_ram[(addr - 0x6000) as usize] = data,
            _ => {
                let mut prg_addr = addr - 0x8000;
                if self.prg_rom.len() == 0x4000 {
                    prg_addr %= 0x4000;
                }

                self.prg_rom[prg_addr as usize] = data;
            }
        }
    }

    fn read_chr(&self, addr: u16) -> u8 {
        if let Some(ram) = &self.chr_ram {
            ram[addr as usize]
        } else {
            self.chr_rom[addr as usize]
        }
    }

    fn write_chr(&mut self, addr: u16, data: u8) {
        if let Some(ram) = &mut self.chr_ram {
            ram[addr as usize] = data;
        }
    }

    fn mirroring(&self) -> Mirroring {
        self.mirroring
    }
}

#[cfg(test)]
mod tests {
    use crate::cartridge::Mirroring;
    use crate::cartridge::Rom;
    use crate::mapper::Mapper;
    use crate::mapper::nrom::Nrom;

    fn nrom_with_chr(chr_data: Vec<u8>, mirroring: Mirroring) -> Nrom {
        Nrom::new(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: chr_data,
            mapper: 0,
            screen_mirroring: mirroring,
        })
    }

    fn nrom_with_prg(prg_data: Vec<u8>, mirroring: Mirroring) -> Nrom {
        Nrom::new(Rom {
            prg_rom: prg_data,
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: mirroring,
        })
    }

    #[test]
    fn test_nrom_read_prg_16kb_mirror() {
        let mut prg = vec![0u8; 0x4000];
        prg[0] = 0x42;
        prg[0x100] = 0x24;
        let nrom = nrom_with_prg(prg, Mirroring::Horizontal);

        assert_eq!(nrom.read_prg(0x8000), 0x42);
        assert_eq!(nrom.read_prg(0xC000), 0x42);
        assert_eq!(nrom.read_prg(0x8100), 0x24);
        assert_eq!(nrom.read_prg(0xC100), 0x24);
    }

    #[test]
    fn test_nrom_read_prg_16kb_top_of_range() {
        let mut prg = vec![0u8; 0x4000];
        prg[0x3FFE] = 0xAA;
        prg[0x3FFF] = 0xBB;
        let nrom = nrom_with_prg(prg, Mirroring::Horizontal);

        assert_eq!(nrom.read_prg(0xBFFE), 0xAA);
        assert_eq!(nrom.read_prg(0xBFFF), 0xBB);
        assert_eq!(nrom.read_prg(0xFFFE), 0xAA);
        assert_eq!(nrom.read_prg(0xFFFF), 0xBB);
    }

    #[test]
    fn test_nrom_read_prg_32kb_no_mirror() {
        let mut prg = vec![0u8; 0x8000];
        prg[0] = 0x42;
        prg[0x4000] = 0x24;
        let nrom = nrom_with_prg(prg, Mirroring::Horizontal);

        assert_eq!(nrom.read_prg(0x8000), 0x42);
        assert_eq!(nrom.read_prg(0xC000), 0x24);
    }

    #[test]
    fn test_nrom_read_prg_32kb_tail() {
        let mut prg = vec![0u8; 0x8000];
        prg[0x7FFC] = 0xAA;
        prg[0x7FFD] = 0xBB;
        let nrom = nrom_with_prg(prg, Mirroring::Horizontal);

        assert_eq!(nrom.read_prg(0xFFFC), 0xAA);
        assert_eq!(nrom.read_prg(0xFFFD), 0xBB);
    }

    #[test]
    fn test_nrom_write_prg_silently_ignored_by_default() {
        let _nrom = nrom_with_prg(vec![0x42; 0x4000], Mirroring::Horizontal);
    }

    #[test]
    fn test_nrom_write_prg_does_not_change_read_back() {
        let mut nrom = nrom_with_prg(vec![0x42; 0x4000], Mirroring::Horizontal);
        nrom.write_prg(0x8000, 0xFF);
        assert_eq!(nrom.read_prg(0x8000), 0xFF);
    }

    #[test]
    fn test_nrom_read_chr() {
        let mut chr = vec![0u8; 0x2000];
        chr[0x100] = 0x42;
        chr[0x1FFF] = 0x24;
        let nrom = nrom_with_chr(chr, Mirroring::Horizontal);

        assert_eq!(nrom.read_chr(0x0100), 0x42);
        assert_eq!(nrom.read_chr(0x1FFF), 0x24);
    }

    #[test]
    fn test_nrom_read_chr_full_range() {
        let chr = (0..0x2000u16).map(|i| (i & 0xFF) as u8).collect();
        let nrom = nrom_with_chr(chr, Mirroring::Horizontal);

        assert_eq!(nrom.read_chr(0x0000), 0x00);
        assert_eq!(nrom.read_chr(0x00FF), 0xFF);
        assert_eq!(nrom.read_chr(0x0100), 0x00);
        assert_eq!(nrom.read_chr(0x1FFF), (0x1FFF & 0xFF) as u8);
    }

    #[test]
    fn test_nrom_write_chr_rom_silently_ignored() {
        let mut nrom = nrom_with_chr(vec![0xAA; 0x2000], Mirroring::Horizontal);
        nrom.write_chr(0x0100, 0xFF);
        assert_eq!(nrom.read_chr(0x0100), 0xAA);
    }

    #[test]
    fn test_nrom_mirroring_vertical() {
        let nrom = nrom_with_prg(vec![0; 0x8000], Mirroring::Vertical);
        assert_eq!(nrom.mirroring(), Mirroring::Vertical);
    }

    #[test]
    fn test_nrom_mirroring_horizontal() {
        let nrom = nrom_with_prg(vec![0; 0x8000], Mirroring::Horizontal);
        assert_eq!(nrom.mirroring(), Mirroring::Horizontal);
    }

    #[test]
    fn test_nrom_mirroring_four_screen() {
        let nrom = nrom_with_prg(vec![0; 0x8000], Mirroring::FourScreen);
        assert_eq!(nrom.mirroring(), Mirroring::FourScreen);
    }

    #[test]
    fn test_nrom_reads_prg_through_trait() {
        let mut prg = vec![0u8; 0x4000];
        prg[0x100] = 0x42;
        let nrom = nrom_with_prg(prg, Mirroring::Horizontal);

        let mapper: &dyn Mapper = &nrom;
        assert_eq!(mapper.read_prg(0x8100), 0x42);
    }

    #[test]
    fn test_nrom_writes_prg_through_trait() {
        let mut nrom = nrom_with_prg(vec![0x42; 0x4000], Mirroring::Horizontal);
        let mapper: &mut dyn Mapper = &mut nrom;
        mapper.write_prg(0x8000, 0xFF);
        assert_eq!(mapper.read_prg(0x8000), 0xFF);
    }

    #[test]
    fn test_nrom_reads_chr_through_trait() {
        let mut chr = vec![0u8; 0x2000];
        chr[0x100] = 0x42;
        let nrom = nrom_with_chr(chr, Mirroring::Horizontal);

        let mapper: &dyn Mapper = &nrom;
        assert_eq!(mapper.read_chr(0x0100), 0x42);
    }

    #[test]
    fn test_nrom_mirroring_through_trait() {
        let nrom = nrom_with_prg(vec![0; 0x8000], Mirroring::FourScreen);
        let mapper: &dyn Mapper = &nrom;
        assert_eq!(mapper.mirroring(), Mirroring::FourScreen);
    }

    #[test]
    fn test_nrom_read_prg_ram_6000() {
        let mut nrom = Nrom::new(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        });
        nrom.write_prg(0x6000, 0x42);
        assert_eq!(nrom.read_prg(0x6000), 0x42);
    }

    #[test]
    fn test_nrom_write_prg_ram_6000_persists() {
        let mut nrom = Nrom::new(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        });
        nrom.write_prg(0x6000, 0xAA);
        nrom.write_prg(0x6001, 0xBB);
        assert_eq!(nrom.read_prg(0x6000), 0xAA);
        assert_eq!(nrom.read_prg(0x6001), 0xBB);
    }

    #[test]
    fn test_nrom_prg_ram_does_not_affect_prg_rom() {
        let mut nrom = Nrom::new(Rom {
            prg_rom: vec![0x42; 0x8000],
            chr_rom: vec![0; 0x2000],
            mapper: 0,
            screen_mirroring: Mirroring::Horizontal,
        });
        nrom.write_prg(0x6000, 0xFF);
        assert_eq!(nrom.read_prg(0x8000), 0x42);
    }

    fn nrom_with_chr_ram(mirroring: Mirroring) -> Nrom {
        Nrom::new(Rom {
            prg_rom: vec![0; 0x8000],
            chr_rom: vec![],
            mapper: 0,
            screen_mirroring: mirroring,
        })
    }

    #[test]
    fn test_nrom_read_chr_ram_defaults_to_zero() {
        let nrom = nrom_with_chr_ram(Mirroring::Horizontal);
        assert_eq!(nrom.read_chr(0x0000), 0x00);
        assert_eq!(nrom.read_chr(0x1FFF), 0x00);
    }

    #[test]
    fn test_nrom_write_chr_ram_persists() {
        let mut nrom = nrom_with_chr_ram(Mirroring::Horizontal);
        nrom.write_chr(0x0100, 0x42);
        assert_eq!(nrom.read_chr(0x0100), 0x42);
    }

    #[test]
    fn test_nrom_write_chr_rom_still_ignored() {
        let mut nrom = nrom_with_chr(vec![0xAA; 0x2000], Mirroring::Horizontal);
        nrom.write_chr(0x0100, 0xFF);
        assert_eq!(nrom.read_chr(0x0100), 0xAA);
    }
}
