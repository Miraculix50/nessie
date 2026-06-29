use bitflags::bitflags;

bitflags! {

    /// Status register
    ///
    ///

    pub struct StatusRegister: u8 {
        const NOTUSED           = 0b00000001;
        const NOTUSED2          = 0b00000010;
        const NOTUSED3          = 0b00000100;
        const NOTUSED4          = 0b00001000;
        const NOTUSED5          = 0b00010000;
        const SPRITE_OVERFLOW   = 0b00100000;
        const SPRITE_0_HIT      = 0b01000000;
        const VBLANK            = 0b10000000;
    }
}

impl StatusRegister {
    pub fn new() -> Self {
        StatusRegister::from_bits_truncate(0b00000000)
    }

    pub fn set_vblank_status(&mut self, status: bool) {
        self.set(StatusRegister::VBLANK, status);
    }

    pub fn set_sprite_zero_hit(&mut self, status: bool) {
        self.set(StatusRegister::SPRITE_0_HIT, status);
    }

    pub fn reset_vblank_status(&mut self) {
        self.remove(StatusRegister::VBLANK);
    }

    pub fn is_in_vblank(&self) -> bool {
        self.contains(StatusRegister::VBLANK)
    }

    pub fn is_sprite_zero_hit(&self) -> bool {
        self.contains(StatusRegister::SPRITE_0_HIT)
    }

    pub fn snapshot(&self) -> u8 {
        self.bits()
    }
}
