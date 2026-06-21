pub struct ScrollRegister {
    pub scroll_x: u8,
    pub scroll_y: u8,
    x_ptr: bool, // Same as hi_ptr of AddressRegister on real hardware
}

impl ScrollRegister {
    pub fn new() -> Self {
        ScrollRegister {
            scroll_x: 0,
            scroll_y: 0,
            x_ptr: true,
        }
    }

    pub fn write(&mut self, value: u8) {
        if self.x_ptr {
            self.scroll_x = value;
        } else {
            self.scroll_y = value;
        }

        self.x_ptr = !self.x_ptr
    }

    pub fn reset_latch(&mut self) {
        self.x_ptr = true;
    }
}
