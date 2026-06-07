use crate::opcodes;

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: u8,
    pub program_counter: u16,
    memory: [u8; 0xFFFF],
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}

trait Mem {
    fn mem_read(&self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}

impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.memory[addr as usize] = data;
    }
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            status: 0,
            program_counter: 0,
            memory: [0; 0xFFFF],
        }
    }

    fn get_operand_address(&mut self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.program_counter,

            AddressingMode::ZeroPage => self.mem_read(self.program_counter) as u16,

            AddressingMode::Absolute => self.mem_read_u16(self.program_counter),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }

            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.program_counter);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_x as u16);
                addr
            }

            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(self.program_counter);
                let addr = base.wrapping_add(self.register_y as u16);
                addr
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.program_counter);

                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }

            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.program_counter);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                deref
            }

            AddressingMode::NoneAddressing => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    // LDA (load to register A)
    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    // LDX (load to register X)
    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
    }

    // LDY (load to register Y)
    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);
    }

    // STA (store register A in memory)
    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    // STX (store register X in memory)
    fn stx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }

    // STY (store register Y in memory)
    fn sty(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_y);
    }

    // TAX (transfer A to X)
    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    // TXA (transfer X to A)
    fn txa(&mut self) {
        self.register_a = self.register_x;
        self.update_zero_and_negative_flags(self.register_a);
    }

    // TAY (transfer A to Y)
    fn tay(&mut self) {
        self.register_y = self.register_a;
        self.update_zero_and_negative_flags(self.register_y);
    }

    // TYA (transfer register Y to register A)
    fn tya(&mut self) {
        self.register_a = self.register_y;
        self.update_zero_and_negative_flags(self.register_a);
    }

    // INX (increment X)
    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    // DEX (decrement register X)
    fn dex(&mut self) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    // INY (increment Y)
    fn iny(&mut self) {
        self.register_y = self.register_y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    // DEY (decrement register Y)
    fn dey(&mut self) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    // SEC (set carry flag)
    fn sec(&mut self) {
        self.status |= 0b0000_0001;
    }

    // CLC (clear carry flag)
    fn clc(&mut self) {
        self.status &= 0b1111_1110;
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.status |= 0b0000_0010; // Set Z flag (last result was 0)
        } else {
            self.status &= 0b1111_1101; // Unset Z flag (last result wasn't 0)
        }

        // First byte of result is set -> result is negative
        if result & 0b1000_0000 != 0 {
            self.status |= 0b1000_0000; // Set N flag (last result was negative)
        } else {
            self.status &= 0b0111_1111; // Unset N flag (last result was positive)
        }
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self, program: Vec<u8>) {
        self.memory[0x8000..(0x8000 + program.len())].copy_from_slice(&program[..]);
        self.mem_write_u16(0xFFFC, 0x8000);
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.status = 0;

        self.program_counter = self.mem_read_u16(0xFFFC)
    }

    pub fn run(&mut self) {
        let opcodes = &*opcodes::OPCODES_MAP;

        loop {
            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;

            let opcode = opcodes
                .get(&code)
                .expect(&format!("OpCode {:x} is not recognized", code));

            match code {
                // LDA (load to register A)
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => {
                    self.lda(&opcode.mode);
                }

                // LDX (load to register X)
                0xa2 | 0xa6 | 0xb6 | 0xae | 0xbe => {
                    self.ldx(&opcode.mode);
                }

                // LDY (load to register Y)
                0xa0 | 0xa4 | 0xb4 | 0xac | 0xbc => {
                    self.ldy(&opcode.mode);
                }

                // STA (store register A in memory)
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => {
                    self.sta(&opcode.mode);
                }

                // STX (store register X in memory)
                0x86 | 0x96 | 0x8e => {
                    self.stx(&opcode.mode);
                }

                // STY (store register Y in memory)
                0x84 | 0x94 | 0x8c => {
                    self.sty(&opcode.mode);
                }

                0xaa => self.tax(), // TAX (transfer register A to register X)
                0x8a => self.txa(), // TXA (transfer register X to register A)
                0xa8 => self.tay(), // TAY (transfer register A to register Y)
                0x98 => self.tya(), // TYA (transfer register Y to register A)
                0xe8 => self.inx(), // INX (increment register X)
                0xca => self.dex(), // DEX (decrement register X)
                0xc8 => self.iny(), // INY (increment register Y)
                0x88 => self.dey(), // DEY (decrement register Y)
                0x38 => self.sec(), // SEC (set carry flag)
                0x18 => self.clc(), // CLC (clear carry flag)
                0xea => {}          // NOP (do nothing, only increment program counter)
                0x00 => return,     // BRK (stop execution)
                _ => unimplemented!(
                    "opcode 0x{:02x} ({}) has no dispatch arm in run()",
                    code,
                    opcode.mnemonic
                ), // A not implemented operation code
            }

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // ----- Memory primitives (Mem trait) -----

    #[test]
    fn test_mem_read_write() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x1234, 0x42);
        assert_eq!(cpu.mem_read(0x1234), 0x42);
    }

    #[test]
    fn test_mem_read_write_u16_little_endian() {
        let mut cpu = CPU::new();
        cpu.mem_write_u16(0x1234, 0xBEEF);
        assert_eq!(cpu.mem_read(0x1234), 0xEF);
        assert_eq!(cpu.mem_read(0x1235), 0xBE);
        assert_eq!(cpu.mem_read_u16(0x1234), 0xBEEF);
    }

    // ----- CPU lifecycle -----

    #[test]
    fn test_reset_clears_registers_and_loads_pc() {
        let mut cpu = CPU::new();
        cpu.register_a = 0xFF;
        cpu.register_x = 0xFF;
        cpu.register_y = 0xFF;
        cpu.status = 0xFF;
        cpu.mem_write_u16(0xFFFC, 0x8000);

        cpu.reset();

        assert_eq!(cpu.register_a, 0);
        assert_eq!(cpu.register_x, 0);
        assert_eq!(cpu.register_y, 0);
        assert_eq!(cpu.status, 0);
        assert_eq!(cpu.program_counter, 0x8000);
    }

    #[test]
    fn test_load_places_program_at_0x8000_and_sets_reset_vector() {
        let mut cpu = CPU::new();
        cpu.load(vec![0xa9, 0x42, 0x00]);

        assert_eq!(cpu.mem_read(0x8000), 0xa9);
        assert_eq!(cpu.mem_read(0x8001), 0x42);
        assert_eq!(cpu.mem_read(0x8002), 0x00);
        assert_eq!(cpu.mem_read_u16(0xFFFC), 0x8000);
    }

    // ----- LDA addressing modes -----

    #[test]
    fn test_lda_immediate() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x42, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_zero_page() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x10, 0x42);
        cpu.load_and_run(vec![0xa5, 0x10, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_zero_page_x_with_wrap() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x01, 0x42);
        // LDA #$02; TAX; LDA $FF,X  -> 0xFF + 2 wraps to 0x01
        cpu.load_and_run(vec![0xa9, 0x02, 0xaa, 0xb5, 0xff, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_absolute() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x1234, 0x42);
        cpu.load_and_run(vec![0xad, 0x34, 0x12, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_absolute_x() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x1236, 0x42);
        // LDA #$02; TAX; LDA $1234,X
        cpu.load_and_run(vec![0xa9, 0x02, 0xaa, 0xbd, 0x34, 0x12, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_absolute_y() {
        // No LDY/TAY instruction exists yet, so set register_y manually after reset.
        let mut cpu = CPU::new();
        cpu.mem_write(0x1236, 0x42);
        cpu.load(vec![0xb9, 0x34, 0x12, 0x00]);
        cpu.reset();
        cpu.register_y = 0x02;
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_indirect_x_with_wrap() {
        let mut cpu = CPU::new();
        // Pointer at zero-page 0x00/0x01 -> effective address 0x1234
        cpu.mem_write(0x00, 0x34);
        cpu.mem_write(0x01, 0x12);
        cpu.mem_write(0x1234, 0x42);
        // LDA #$02; TAX; LDA ($FE,X)  -> 0xFE + 2 wraps to 0x00
        cpu.load_and_run(vec![0xa9, 0x02, 0xaa, 0xa1, 0xfe, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_indirect_y() {
        let mut cpu = CPU::new();
        // Pointer at zero-page 0x10/0x11 -> base 0x1234, +Y(2) = 0x1236
        cpu.mem_write(0x10, 0x34);
        cpu.mem_write(0x11, 0x12);
        cpu.mem_write(0x1236, 0x42);
        cpu.load(vec![0xb1, 0x10, 0x00]);
        cpu.reset();
        cpu.register_y = 0x02;
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    // ----- Zero & Negative flags (tested via LDA) -----

    #[test]
    fn test_lda_sets_zero_flag_when_zero() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x00, 0x00]);
        assert_eq!(
            cpu.status & 0b0000_0010,
            0b0000_0010,
            "Z flag should be set"
        );
        assert_eq!(cpu.status & 0b1000_0000, 0, "N flag should be clear");
    }

    #[test]
    fn test_lda_sets_negative_flag_when_high_bit_set() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0x00]);
        assert_eq!(
            cpu.status & 0b1000_0000,
            0b1000_0000,
            "N flag should be set"
        );
        assert_eq!(cpu.status & 0b0000_0010, 0, "Z flag should be clear");
    }

    // ----- STA -----

    #[test]
    fn test_sta_writes_a_to_memory() {
        let mut cpu = CPU::new();
        // LDA #$42; STA $10
        cpu.load_and_run(vec![0xa9, 0x42, 0x85, 0x10, 0x00]);
        assert_eq!(cpu.mem_read(0x10), 0x42);
    }

    // ----- LDX -----

    #[test]
    fn test_ldx_immediate() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa2, 0x42, 0x00]);
        assert_eq!(cpu.register_x, 0x42);
    }

    #[test]
    fn test_ldx_zero_page_y_with_wrap() {
        let mut cpu = CPU::new();
        cpu.mem_write(0x01, 0x42);
        // LDA #$02; TAY; LDX $FF,Y  -> 0xFF + 2 wraps to 0x01
        cpu.load_and_run(vec![0xa9, 0x02, 0xa8, 0xb6, 0xff, 0x00]);
        assert_eq!(cpu.register_x, 0x42);
    }

    // ----- LDY -----

    #[test]
    fn test_ldy_immediate() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa0, 0x42, 0x00]);
        assert_eq!(cpu.register_y, 0x42);
    }

    // ----- STX -----

    #[test]
    fn test_stx_writes_x_to_memory() {
        let mut cpu = CPU::new();
        // LDA #$42; TAX; STX $10; BRK
        // TAX is needed because X starts at 0 after reset — without it we could
        // not distinguish a working STX from a broken one that writes 0.
        cpu.load_and_run(vec![0xa9, 0x42, 0xaa, 0x86, 0x10, 0x00]);
        assert_eq!(cpu.mem_read(0x10), 0x42);
    }

    // ----- STY -----

    #[test]
    fn test_sty_writes_y_to_memory() {
        let mut cpu = CPU::new();
        // LDA #$42; TAY; STY $10; BRK
        // TAY is needed because Y starts at 0 after reset — without it we could
        // not distinguish a working STY from a broken one that writes 0.
        cpu.load_and_run(vec![0xa9, 0x42, 0xa8, 0x84, 0x10, 0x00]);
        assert_eq!(cpu.mem_read(0x10), 0x42);
    }

    // ----- CLC & SEC (Carry flag) -----

    #[test]
    fn test_sec_sets_carry_flag() {
        let mut cpu = CPU::new();
        // C starts at 0 after reset; SEC must set bit 0 of the status register.
        cpu.load_and_run(vec![0x38, 0x00]);
        assert_eq!(
            cpu.status & 0b0000_0001,
            0b0000_0001,
            "C flag should be set"
        );
    }

    #[test]
    fn test_clc_clears_carry_flag() {
        let mut cpu = CPU::new();
        // SEC; CLC; BRK -- C is set then cleared, exercising both transitions.
        cpu.load_and_run(vec![0x38, 0x18, 0x00]);
        assert_eq!(cpu.status & 0b0000_0001, 0, "C flag should be clear");
    }

    // ----- AND -----

    #[test]
    fn test_and_immediate() {
        let mut cpu = CPU::new();
        // LDA #$AA; AND #$55 -> 0xAA & 0x55 = 0x00, Z flag must be set.
        // This single test exercises: opcode wiring, the AND operation, storing
        // the result back into A, and Z-flag handling (a buggy AND that forgot
        // to call update_zero_and_negative_flags would fail the Z assertion).
        cpu.load_and_run(vec![0xa9, 0xaa, 0x29, 0x55, 0x00]);
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(
            cpu.status & 0b0000_0010,
            0b0000_0010,
            "Z flag should be set when AND result is 0"
        );
        assert_eq!(cpu.status & 0b1000_0000, 0, "N flag should be clear");
    }

    // ----- Register transfers & increments -----

    #[test]
    fn test_tax_transfers_a_to_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x42, 0xaa, 0x00]);
        assert_eq!(cpu.register_x, 0x42);
    }

    #[test]
    fn test_tay_transfers_a_to_y() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x42, 0xa8, 0x00]);
        assert_eq!(cpu.register_y, 0x42);
    }

    #[test]
    fn test_txa_transfers_x_to_a() {
        let mut cpu = CPU::new();
        // LDA #$42; TAX; LDA #$00; TXA; BRK
        // A is overwritten between TAX and TXA so the assertion proves X -> A actually happened.
        cpu.load_and_run(vec![0xa9, 0x42, 0xaa, 0xa9, 0x00, 0x8a, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_tya_transfers_y_to_a() {
        let mut cpu = CPU::new();
        // LDA #$42; TAY; LDA #$00; TYA; BRK
        // A is overwritten between TAY and TYA so the assertion proves Y -> A actually happened.
        cpu.load_and_run(vec![0xa9, 0x42, 0xa8, 0xa9, 0x00, 0x98, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_inx_increments_x() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x05, 0xaa, 0xe8, 0x00]);
        assert_eq!(cpu.register_x, 0x06);
    }

    #[test]
    fn test_inx_overflow_wraps_to_zero() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0xff, 0xaa, 0xe8, 0x00]);
        assert_eq!(cpu.register_x, 0x00);
        assert_eq!(
            cpu.status & 0b0000_0010,
            0b0000_0010,
            "Z flag should be set on wrap"
        );
    }

    #[test]
    fn test_iny_increments_y() {
        let mut cpu = CPU::new();
        // LDA #$05; TAY; INY; BRK
        cpu.load_and_run(vec![0xa9, 0x05, 0xa8, 0xc8, 0x00]);
        assert_eq!(cpu.register_y, 0x06);
    }

    #[test]
    fn test_dex_underflow_wraps_to_0xff() {
        let mut cpu = CPU::new();
        // X starts at 0 after reset; DEX -> 0xFF (underflow), sets N flag.
        cpu.load_and_run(vec![0xca, 0x00]);
        assert_eq!(cpu.register_x, 0xff);
        assert_eq!(
            cpu.status & 0b1000_0000,
            0b1000_0000,
            "N flag should be set on underflow"
        );
        assert_eq!(cpu.status & 0b0000_0010, 0, "Z flag should be clear");
    }

    #[test]
    fn test_dey_decrements_y() {
        let mut cpu = CPU::new();
        // LDA #$05; TAY; DEY; BRK
        cpu.load_and_run(vec![0xa9, 0x05, 0xa8, 0x88, 0x00]);
        assert_eq!(cpu.register_y, 0x04);
    }

    // ----- NOP -----

    #[test]
    fn test_nop_does_nothing() {
        let mut cpu = CPU::new();
        // LDA #$42; NOP; BRK -- NOP must not modify A
        cpu.load_and_run(vec![0xa9, 0x42, 0xea, 0x00]);
        assert_eq!(cpu.register_a, 0x42);
    }

    // ----- Integration -----

    #[test]
    fn test_lda_tax_inx_program() {
        let mut cpu = CPU::new();
        // LDA #$c0; TAX; INX; BRK
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);
        assert_eq!(cpu.register_x, 0xc1);
        assert_eq!(
            cpu.status & 0b1000_0000,
            0b1000_0000,
            "N flag should be set (0xc1 is negative)"
        );
        assert_eq!(cpu.status & 0b0000_0010, 0, "Z flag should be clear");
    }
}
