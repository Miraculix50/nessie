use bitflags::bitflags;

bitflags! {

    /// Control register
    ///
    /// 7 6 5 4 3 2 1 0
    /// V P H B S I N N
    /// | | | | | | | |
    /// | | | | | | +–+–– Base nametable address
    /// | | | | | |       (0 = $2000; 1 = $2400; 2 = $2800; 3 = $2C00)
    /// | | | | | +–––––– VRAM address increment per CPU read/write of PPUDATA
    /// | | | | |         (0 = +1, going accross; 1 = +32, going down)
    /// | | | | +–––––––– Sprite pattern table address for 8x8 sprites
    /// | | | |           (0 = $0000; 1 = $1000; ignored in 8x16 mode)
    /// | | | +–––––––––– Background pattern table address (0 = $0000; 1 = $1000)
    /// | | +–––––––––––– Sprite size (0 = 8x8 pixels; 1 = 8x16 pixels)
    /// | +–––––––––––––– PPU master/slave select
    /// |                 (0 = read backdrop from EXT pins; 1 = output color on EXT pins)
    /// +–––––––––––––––– Vblank NMI enable (0 = off; 1 = on)

    pub struct ControlRegister: u8 {
        const NAMETABLE1                = 0b00000001;
        const NAMETABLE2                = 0b00000010;
        const VRAM_ADD_INCREMENT        = 0b00000100;
        const SPRITE_PATTERN_ADDR       = 0b00001000;
        const BACKGROUND_PATTERN_ADDR   = 0b00010000;
        const SPRITE_SIZE               = 0b00100000;
        const MASTER_SLAVE_SELECT       = 0b01000000;
        const GENERATE_NMI              = 0b10000000;
    }
}

impl ControlRegister {
    pub fn new() -> Self {
        ControlRegister::from_bits_truncate(0b00000000)
    }

    pub fn vram_addr_increment(&self) -> u8 {
        if !self.contains(ControlRegister::VRAM_ADD_INCREMENT) {
            1 // going accross
        } else {
            32 // going down
        }
    }

    pub fn update(&mut self, data: u8) {
        *self = Self::from_bits_truncate(data);
    }

    pub fn generate_vblank_nmi(&self) -> bool {
        self.contains(ControlRegister::GENERATE_NMI)
    }

    pub fn background_pattern_addr(&self) -> u16 {
        if !self.contains(ControlRegister::BACKGROUND_PATTERN_ADDR) {
            0
        } else {
            0x1000
        }
    }

    pub fn sprite_pattern_addr(&self) -> u16 {
        if !self.contains(ControlRegister::SPRITE_PATTERN_ADDR) {
            0
        } else {
            0x1000
        }
    }

    pub fn sprite_size(&self) -> u8 {
        if !self.contains(ControlRegister::SPRITE_SIZE) {
            8
        } else {
            16
        }
    }
}
