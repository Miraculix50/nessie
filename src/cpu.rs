use crate::opcodes;
use bitflags::bitflags;

bitflags! {

    /// Status register
    ///
    /// 7 6 5 4 3 2 1 0
    /// N V _ B D I Z C
    /// | |   | | | | +––– Carry flag
    /// | |   | | | +––––– Zero flag
    /// | |   | | +––––––– Interrupt disable
    /// | |   | +––––––––– Decimal mode (not used on NES)
    /// | |   +––––––––––– Break command
    /// | +––––––––––––––– Overflow flag
    /// +––––––––––––––––– Negative flag

    #[derive(Debug)]
    pub struct CPUFlags: u8 {
        const CARRY             = 0b00000001;
        const ZERO              = 0b00000010;
        const INTERRUPT_DISABLE = 0b00000100;
        const DECIMAL_MODE      = 0b00001000;
        const BREAK             = 0b00010000;
        const BREAK2            = 0b00100000;
        const OVERFLOW          = 0b01000000;
        const NEGATIVE          = 0b10000000;
    }
}

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: CPUFlags,
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
        (hi << 8) | lo
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
            status: CPUFlags::from_bits_truncate(0b00100100), // Set BREAK2 and INTERRUPT_DISABLE
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

    /// Update zero and negative flags of the status register
    fn update_zero_and_negative_flags(&mut self, result: u8) {
        // Set Z flag if result was zero
        self.status.set(CPUFlags::ZERO, result == 0);

        // First byte of result is set -> result is negative
        self.status
            .set(CPUFlags::NEGATIVE, result & 0b1000_0000 != 0);
    }

    /// Sets the register A of this [`CPU`] and updates status flags
    fn set_register_a(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    /// Sets the register X of this [`CPU`] and updates status flags
    fn set_register_x(&mut self, value: u8) {
        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
    }

    /// Sets the register Y of this [`CPU`] and updates status flags
    fn set_register_y(&mut self, value: u8) {
        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);
    }

    /// Helper function to perform an addition on register A
    fn add_to_register_a(&mut self, value: u8) {
        let carry = self.status.contains(CPUFlags::CARRY) as u16;
        let result_u16 = self.register_a as u16 + value as u16 + carry;

        self.status.set(CPUFlags::CARRY, result_u16 > 0xFF); // Set C flag if result has a carry-out

        let result_u8 = (result_u16 & 0xFF) as u8;
        let v = ((self.register_a ^ result_u8) & (value ^ result_u8) & 0x80) != 0;
        self.status.set(CPUFlags::OVERFLOW, v); // Set V flag if signed result overflowed

        self.set_register_a(result_u8);
    }

    /// LDA (load to register A)
    fn lda(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(value);
    }

    /// LDX (load to register X)
    fn ldx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_x(value);
    }

    /// LDY (load to register Y)
    fn ldy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_y(value);
    }

    /// STA (store register A in memory)
    fn sta(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    /// STX (store register X in memory)
    fn stx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_x);
    }

    /// STY (store register Y in memory)
    fn sty(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        self.mem_write(addr, self.register_y);
    }

    /// TAX (transfer A to X)
    fn tax(&mut self) {
        self.set_register_x(self.register_a);
    }

    /// TXA (transfer X to A)
    fn txa(&mut self) {
        self.set_register_a(self.register_x);
    }

    /// TAY (transfer A to Y)
    fn tay(&mut self) {
        self.set_register_y(self.register_a);
    }

    /// TYA (transfer register Y to register A)
    fn tya(&mut self) {
        self.set_register_a(self.register_y);
    }

    /// INX (increment X)
    fn inx(&mut self) {
        self.set_register_x(self.register_x.wrapping_add(1));
    }

    /// DEX (decrement register X)
    fn dex(&mut self) {
        self.set_register_x(self.register_x.wrapping_sub(1));
    }

    /// INY (increment Y)
    fn iny(&mut self) {
        self.set_register_y(self.register_y.wrapping_add(1));
    }

    /// DEY (decrement register Y)
    fn dey(&mut self) {
        self.set_register_y(self.register_y.wrapping_sub(1));
    }

    /// SEC (set carry flag)
    fn sec(&mut self) {
        self.status.insert(CPUFlags::CARRY);
    }

    /// CLC (clear carry flag)
    fn clc(&mut self) {
        self.status.remove(CPUFlags::CARRY);
    }

    /// SED (set decimal flag)
    fn sed(&mut self) {
        self.status.insert(CPUFlags::DECIMAL_MODE);
    }

    /// CLD (clear decimal flag)
    fn cld(&mut self) {
        self.status.remove(CPUFlags::DECIMAL_MODE);
    }

    /// SEI (set interrupt disable flag)
    fn sei(&mut self) {
        self.status.insert(CPUFlags::INTERRUPT_DISABLE);
    }

    /// CLI (clear interrupt disable flag)
    fn cli(&mut self) {
        self.status.remove(CPUFlags::INTERRUPT_DISABLE);
    }

    /// CLV (clear overflow flag)
    fn clv(&mut self) {
        self.status.remove(CPUFlags::OVERFLOW);
    }

    /// AND (perform a logical AND on register A)
    fn and(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(self.register_a & value);
    }

    /// ORA (perform a logical OR on register A)
    fn ora(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(self.register_a | value);
    }

    /// EOR (perform a logical XOR on register A)
    fn eor(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(self.register_a ^ value);
    }

    /// CMP (compare register A with memory)
    fn cmp(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let result = self.register_a.wrapping_sub(value);
        self.update_zero_and_negative_flags(result);
        // Set C flag when register A was bigger than value
        self.status.set(CPUFlags::CARRY, self.register_a >= value);
    }

    /// ADC (add to register A with carry-in)
    fn adc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.add_to_register_a(value);
    }

    /// SBC (substract from register A with borrow-in)
    fn sbc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.add_to_register_a(value ^ 0xFF);
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
        self.status = CPUFlags::from_bits_truncate(0b00100100);

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
                .expect(&format!("OpCode 0x{:02x} is not recognized", code));

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

                // AND (perform a logical AND on register A)
                0x29 | 0x25 | 0x35 | 0x2d | 0x3d | 0x39 | 0x21 | 0x31 => {
                    self.and(&opcode.mode);
                }

                // ORA (perform a logical OR on register A)
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => {
                    self.ora(&opcode.mode);
                }

                // EOR (perform a logical XOR on register A)
                0x49 | 0x45 | 0x55 | 0x4d | 0x5d | 0x59 | 0x41 | 0x51 => {
                    self.eor(&opcode.mode);
                }

                // CMP (compare register A with memory)
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => {
                    self.cmp(&opcode.mode);
                }

                // ADC (add to register A with carry-in)
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => {
                    self.adc(&opcode.mode);
                }

                // SBC (substract from register A with borrow-in)
                0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => {
                    self.sbc(&opcode.mode);
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
                0xf8 => self.sed(), // SED (set decimal flag)
                0xd8 => self.cld(), // CLD (clear decimal flag)
                0x78 => self.sei(), // SEI (set interrupt disable flag)
                0x58 => self.cli(), // CLI (clear interrupt disable flag)
                0xb8 => self.clv(), // CLV (clear overflow flag)
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
        cpu.status = CPUFlags::from_bits_truncate(0xFF);
        cpu.mem_write_u16(0xFFFC, 0x8000);

        cpu.reset();

        assert_eq!(cpu.register_a, 0);
        assert_eq!(cpu.register_x, 0);
        assert_eq!(cpu.register_y, 0);
        assert_eq!(
            cpu.status.bits(),
            0b00100100,
            "Status should have BREAK2 and INTERRUPT_DISABLE set after reset"
        );
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
        assert!(cpu.status.contains(CPUFlags::ZERO), "Z flag should be set");
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be clear"
        );
    }

    #[test]
    fn test_lda_sets_negative_flag_when_high_bit_set() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xa9, 0x80, 0x00]);
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be set"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be clear"
        );
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
        assert!(cpu.status.contains(CPUFlags::CARRY), "C flag should be set");
    }

    #[test]
    fn test_clc_clears_carry_flag() {
        let mut cpu = CPU::new();
        // SEC; CLC; BRK -- C is set then cleared, exercising both transitions.
        cpu.load_and_run(vec![0x38, 0x18, 0x00]);
        assert!(
            !cpu.status.contains(CPUFlags::CARRY),
            "C flag should be clear"
        );
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
        assert!(
            cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be set when AND result is 0"
        );
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be clear"
        );
    }

    // ----- CMP -----

    #[test]
    fn test_cmp_equal_sets_z_and_c() {
        let mut cpu = CPU::new();
        // LDA #$05; CMP #$05 -> A - M = 0, so Z=1, C=1 (A >= M).
        // Register A must be unchanged (CMP is a side-effect-free comparison).
        cpu.load_and_run(vec![0xa9, 0x05, 0xc9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 0x05, "CMP must not modify A");
        assert!(
            cpu.status.contains(CPUFlags::CARRY | CPUFlags::ZERO),
            "Z and C flags should both be set when A == M"
        );
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be clear"
        );
    }

    #[test]
    fn test_cmp_less_than_with_carry_set_clears_c() {
        let mut cpu = CPU::new();
        // SEC first so C starts at 1: a buggy CMP that fails to update C
        // (or sets it backwards) would fail the final C==0 assertion.
        // Then LDA #$00; CMP #$01 -> 0 - 1 = 0xFF (borrow), so C=0, Z=0, N=1.
        cpu.load_and_run(vec![0x38, 0xa9, 0x00, 0xc9, 0x01, 0x00]);
        assert_eq!(cpu.register_a, 0x00, "CMP must not modify A");
        assert!(
            !cpu.status.contains(CPUFlags::CARRY),
            "C flag should be clear when A < M"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be clear"
        );
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be set (result 0xFF has high bit)"
        );
    }

    // ----- ADC -----

    // Status bit positions used by the ADC tests:
    //   C = bit 0 (0b0000_0001) -- carry out of bit 7 (unsigned overflow)
    //   Z = bit 1 (0b0000_0010) -- result is zero
    //   V = bit 6 (0b0100_0000) -- signed overflow
    //   N = bit 7 (0b1000_0000) -- bit 7 of result

    #[test]
    fn test_adc_basic_add_no_carry_in() {
        let mut cpu = CPU::new();
        // C starts at 0 after reset. 5 + 3 = 8, no overflow anywhere.
        cpu.load_and_run(vec![0xa9, 0x05, 0x69, 0x03, 0x00]);
        assert_eq!(cpu.register_a, 0x08);
        assert!(!cpu.status.contains(CPUFlags::CARRY), "C should be clear");
        assert!(!cpu.status.contains(CPUFlags::ZERO), "Z should be clear");
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear"
        );
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be clear"
        );
    }

    #[test]
    fn test_adc_adds_carry_in() {
        let mut cpu = CPU::new();
        // SEC; LDA #$05; ADC #$03 -> 5 + 3 + 1 = 9.
        // A buggy ADC that ignored the carry-in would produce 8, not 9.
        cpu.load_and_run(vec![0x38, 0xa9, 0x05, 0x69, 0x03, 0x00]);
        assert_eq!(cpu.register_a, 0x09, "carry-in must be added");
        assert!(!cpu.status.contains(CPUFlags::CARRY), "C should be clear");
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear"
        );
    }

    #[test]
    fn test_adc_carry_out_sets_c_and_zero_result_sets_z() {
        let mut cpu = CPU::new();
        // LDA #$FF; ADC #$01 -> 0xFF + 0x01 = 0x100, so A=0x00, C=1, Z=1.
        cpu.load_and_run(vec![0xa9, 0xff, 0x69, 0x01, 0x00]);
        assert_eq!(cpu.register_a, 0x00);
        assert!(
            cpu.status.contains(CPUFlags::CARRY),
            "C should be set on unsigned overflow"
        );
        assert!(
            cpu.status.contains(CPUFlags::ZERO),
            "Z should be set when result is 0"
        );
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear (no signed overflow)"
        );
    }

    #[test]
    fn test_adc_signed_overflow_sets_v() {
        let mut cpu = CPU::new();
        // LDA #$50; ADC #$50 -> 0x50 + 0x50 = 0xA0.
        // In signed: (+80) + (+80) = (-96) is an overflow, so V must be set.
        // 0xA0 also has bit 7 set, so N must be set.
        cpu.load_and_run(vec![0xa9, 0x50, 0x69, 0x50, 0x00]);
        assert_eq!(cpu.register_a, 0xa0);
        assert!(
            cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be set on signed overflow"
        );
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be set (result 0xA0 has high bit)"
        );
        assert!(!cpu.status.contains(CPUFlags::CARRY), "C should be clear");
        assert!(!cpu.status.contains(CPUFlags::ZERO), "Z should be clear");
    }

    // ----- SBC -----

    // SBC semantics: A = A - M - !C. C acts as "no borrow" (inverted from what
    // you might expect); C_in=1 means no borrow coming in, so the full
    // subtraction happens. After the operation, C=1 means no borrow occurred.
    //
    // Status bit positions:
    //   C = bit 0 (0b0000_0001) -- set if no borrow (analogous to ADC's carry-out)
    //   Z = bit 1 (0b0000_0010) -- result is zero
    //   V = bit 6 (0b0100_0000) -- signed overflow on subtraction
    //   N = bit 7 (0b1000_0000) -- bit 7 of result

    #[test]
    fn test_sbc_basic_subtract_no_borrow_in() {
        let mut cpu = CPU::new();
        // SEC; LDA #$05; SBC #$03 -> 5 - 3 - 0 = 2, no borrow anywhere.
        cpu.load_and_run(vec![0x38, 0xa9, 0x05, 0xe9, 0x03, 0x00]);
        assert_eq!(cpu.register_a, 0x02);
        assert!(
            cpu.status.contains(CPUFlags::CARRY),
            "C should be set (no borrow)"
        );
        assert!(!cpu.status.contains(CPUFlags::ZERO), "Z should be clear");
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear"
        );
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be clear"
        );
    }

    #[test]
    fn test_sbc_subtracts_borrow_in() {
        let mut cpu = CPU::new();
        // CLC; LDA #$05; SBC #$03 -> 5 - 3 - 1 = 1.
        // A buggy SBC that ignored the carry-in would produce 2, not 1.
        cpu.load_and_run(vec![0x18, 0xa9, 0x05, 0xe9, 0x03, 0x00]);
        assert_eq!(cpu.register_a, 0x01, "borrow-in must be subtracted");
        assert!(
            cpu.status.contains(CPUFlags::CARRY),
            "C should be set (no borrow)"
        );
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear"
        );
    }

    #[test]
    fn test_sbc_underflow_clears_c_and_sets_n() {
        let mut cpu = CPU::new();
        // SEC; LDA #$00; SBC #$01 -> 0 - 1 - 0 = 0xFF (borrow occurred).
        // V should be clear: 0 - 1 = -1, which is valid in signed 8-bit.
        cpu.load_and_run(vec![0x38, 0xa9, 0x00, 0xe9, 0x01, 0x00]);
        assert_eq!(cpu.register_a, 0xff);
        assert!(
            !cpu.status.contains(CPUFlags::CARRY),
            "C should be clear (borrow occurred)"
        );
        assert!(!cpu.status.contains(CPUFlags::ZERO), "Z should be clear");
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear"
        );
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be set (result 0xFF has high bit)"
        );
    }

    #[test]
    fn test_sbc_signed_overflow_sets_v() {
        let mut cpu = CPU::new();
        // SEC; LDA #$80; SBC #$01 -> 0x80 - 0x01 - 0 = 0x7F.
        // In signed: (-128) - (+1) = -129, which doesn't fit in 8-bit signed,
        // so signed overflow occurred and V must be set.
        // 0x7F has bit 7 clear, so N must be clear.
        cpu.load_and_run(vec![0x38, 0xa9, 0x80, 0xe9, 0x01, 0x00]);
        assert_eq!(cpu.register_a, 0x7f);
        assert!(
            cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be set on signed overflow"
        );
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be clear"
        );
        assert!(
            cpu.status.contains(CPUFlags::CARRY),
            "C should be set (no borrow)"
        );
        assert!(!cpu.status.contains(CPUFlags::ZERO), "Z should be clear");
    }

    // ----- ORA -----

    #[test]
    fn test_ora_immediate() {
        let mut cpu = CPU::new();
        // LDA #$AA; ORA #$55 -> 0xAA | 0x55 = 0xFF, N flag set, Z flag clear.
        // Like AND, this is a direct-mirror pattern: read operand from memory
        // via get_operand_address, OR with A, update flags.
        cpu.load_and_run(vec![0xa9, 0xaa, 0x09, 0x55, 0x00]);
        assert_eq!(cpu.register_a, 0xff);
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be set"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be clear"
        );
    }

    // ----- EOR -----

    #[test]
    fn test_eor_immediate() {
        let mut cpu = CPU::new();
        // LDA #$AA; EOR #$FE -> 0xAA ^ 0xFE = 0x54.
        // Picked so the result differs from OR (0xFE) and AND (0xAA),
        // proving XOR is wired and not accidentally aliased to another op.
        cpu.load_and_run(vec![0xa9, 0xaa, 0x49, 0xfe, 0x00]);
        assert_eq!(cpu.register_a, 0x54);
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be clear"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be clear"
        );
    }

    // ----- CLD & SED (Decimal flag) -----

    // Decimal flag (D) is bit 3 of the status register (0b0000_1000).
    // On the NES's 2A03 CPU the D flag is vestigial (decimal mode is disabled
    // in hardware), but software still uses CLD/SED to save/restore state.
    // The flag must still be read/writable at the status-register level.

    #[test]
    fn test_sed_sets_decimal_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0xf8, 0x00]);
        assert!(
            cpu.status.contains(CPUFlags::DECIMAL_MODE),
            "D flag should be set"
        );
    }

    #[test]
    fn test_cld_clears_decimal_flag() {
        let mut cpu = CPU::new();
        // SED; CLD — exercises both the set and the clear transition.
        cpu.load_and_run(vec![0xf8, 0xd8, 0x00]);
        assert!(
            !cpu.status.contains(CPUFlags::DECIMAL_MODE),
            "D flag should be clear"
        );
    }

    // ----- CLI & SEI (Interrupt disable flag) -----

    // Interrupt disable flag (I) is bit 2 of the status register (0b0000_0100).
    // When I=1 the CPU ignores maskable interrupts (IRQ).
    // Currently unused by the emulator but a prerequisite for BRK/IRQ handling.

    #[test]
    fn test_sei_sets_interrupt_flag() {
        let mut cpu = CPU::new();
        cpu.load_and_run(vec![0x78, 0x00]);
        assert!(
            cpu.status.contains(CPUFlags::INTERRUPT_DISABLE),
            "I flag should be set"
        );
    }

    #[test]
    fn test_cli_clears_interrupt_flag() {
        let mut cpu = CPU::new();
        // SEI; CLI — exercises both transitions.
        cpu.load_and_run(vec![0x78, 0x58, 0x00]);
        assert!(
            !cpu.status.contains(CPUFlags::INTERRUPT_DISABLE),
            "I flag should be clear"
        );
    }

    // ----- CLV (Overflow flag) -----

    // Overflow flag (V) is bit 6 of the status register (0b0100_0000).
    // CLV is the only "clear-only" flag instruction — there is no SEV.
    // To verify CLV works, we first need V=1, which we get from ADC overflow.

    #[test]
    fn test_clv_clears_overflow_flag() {
        let mut cpu = CPU::new();
        // LDA #$50; ADC #$50 -> V=1 (signed overflow), then CLV -> V=0.
        // N and Z should be unchanged by CLV (proving it's not a sledgehammer
        // that clears the whole status register).
        cpu.load_and_run(vec![0xa9, 0x50, 0x69, 0x50, 0xb8, 0x00]);
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V flag should be clear"
        );
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should survive CLV"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should survive CLV"
        );
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
        assert!(
            cpu.status.contains(CPUFlags::ZERO),
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
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be set on underflow"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be clear"
        );
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
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be set (0xc1 is negative)"
        );
        assert!(
            !cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be clear"
        );
    }
}
