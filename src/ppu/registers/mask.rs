use ::bitflags::bitflags;

bitflags! {

    /// Mask register
    ///
    /// 7 6 5 4 3 2 1 0
    /// B G R s b M m G
    /// | | | | | | | |
    /// | | | | | | | +–– Greyscale (0 = normal color; 1 = greyscale)
    /// | | | | | | +–––– 1 = Show background in leftmost 8 pixels of screen; 0 = hide
    /// | | | | | +–––––– 1 = Show sprites in leftmost 8 pixels of screen; 0 = hide
    /// | | | | +–––––––– 1 = Enable background rendering
    /// | | | +–––––––––– 1 = Enable sprite rendering
    /// | | +–––––––––––– Emphasize red (green on PAL/Dendy)
    /// | +–––––––––––––– Emphasize green (red on PAL/Dendy)
    /// +–––––––––––––––– Emphasize blue

    pub struct MaskRegister: u8 {
        const GREYSCALE                 = 0b00000001;
        const LEFTMOST_8PXL_BACKGROUND  = 0b00000010;
        const LEFTMOST_8PXL_SPRITE      = 0b00000100;
        const ENABLE_BACKGROUND         = 0b00001000;
        const ENABLE_SPRITE             = 0b00010000;
        const EMPHASIZE_RED             = 0b00100000;
        const EMPHASIZE_GREEN           = 0b01000000;
        const EMPHASIZE_BLUE            = 0b10000000;
    }
}

impl MaskRegister {
    pub fn new() -> Self {
        MaskRegister::from_bits_truncate(0b00000000)
    }

    pub fn update(&mut self, data: u8) {
        *self = MaskRegister::from_bits_truncate(data);
    }

    pub fn show_background(&self) -> bool {
        self.contains(MaskRegister::ENABLE_BACKGROUND)
    }

    pub fn show_sprites(&self) -> bool {
        self.contains(MaskRegister::ENABLE_SPRITE)
    }
}
