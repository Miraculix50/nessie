use crate::bus::Bus;
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

const STACK: u16 = 0x0100;
const STACK_RESET: u8 = 0xfd;

pub struct CPU {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: CPUFlags,
    pub program_counter: u16,
    pub stack_pointer: u8,

    pub bus: Bus,
    cycle_debt: u8,
    page_crossed: bool,
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

pub trait Mem {
    fn mem_read(&mut self, addr: u16) -> u8;
    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
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
    fn mem_read(&mut self, addr: u16) -> u8 {
        self.bus.mem_read(addr)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.mem_write(addr, data);
    }

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        self.bus.mem_read_u16(pos)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        self.bus.mem_write_u16(pos, data);
    }
}

impl CPU {
    pub fn new(bus: Bus) -> Self {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            status: CPUFlags::from_bits_truncate(0b00100100), // Set BREAK2 and INTERRUPT_DISABLE
            program_counter: 0x8000,
            stack_pointer: STACK_RESET,
            bus: bus,
            cycle_debt: 0,
            page_crossed: false,
        }
    }

    pub fn get_absolute_address(&mut self, mode: &AddressingMode, addr: u16) -> u16 {
        match mode {
            AddressingMode::ZeroPage => self.mem_read(addr) as u16,

            AddressingMode::Absolute => self.mem_read_u16(addr),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(addr);
                let addr = pos.wrapping_add(self.register_x) as u16;
                addr
            }

            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(addr);
                let addr = pos.wrapping_add(self.register_y) as u16;
                addr
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(addr);
                let addr = base.wrapping_add(self.register_x as u16);

                if base & 0xFF00 != addr & 0xFF00 {
                    self.page_crossed = true;
                }

                addr
            }

            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(addr);
                let addr = base.wrapping_add(self.register_y as u16);

                if base & 0xFF00 != addr & 0xFF00 {
                    self.page_crossed = true;
                }

                addr
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(addr);

                let ptr = base.wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16) << 8 | (lo as u16)
            }

            AddressingMode::Indirect_Y => {
                let base = self.mem_read(addr);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read(base.wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);

                if deref_base & 0xFF00 != deref & 0xFF00 {
                    self.page_crossed = true;
                }

                deref
            }

            _ => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    fn get_operand_address(&mut self, mode: &AddressingMode) -> u16 {
        match mode {
            AddressingMode::Immediate => self.program_counter,
            _ => self.get_absolute_address(&mode, self.program_counter),
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

    /// Helper function to perform a subtraction on register A
    /// Uses [`add_to_register_a()`] internally
    fn sub_from_register_a(&mut self, value: u8) {
        self.add_to_register_a(value ^ 0xFF);
    }

    /// Helper function to perform an AND with register A
    fn and_with_register_a(&mut self, value: u8) {
        self.set_register_a(value & self.register_a);
    }

    /// Helper function to perform an XOR with register A
    fn xor_with_register_a(&mut self, value: u8) {
        self.set_register_a(value ^ self.register_a);
    }

    /// Helper function to perform an OR with register A
    fn or_with_register_a(&mut self, value: u8) {
        self.set_register_a(value | self.register_a);
    }

    /// General function for branch opcodes
    fn branch(&mut self, condition: bool) {
        if condition {
            self.cycle_debt += 1;
            let jump = self.mem_read(self.program_counter) as i8 as i16;
            let jump_addr = self
                .program_counter
                .wrapping_add(1)
                .wrapping_add_signed(jump);

            let old_pc = self.program_counter.wrapping_add(1);
            self.program_counter = jump_addr;
            if old_pc & 0xFF00 != self.program_counter & 0xFF00 {
                self.cycle_debt += 1;
            }
        }
    }

    /// Helper function for pushing to the stack
    fn stack_push(&mut self, data: u8) {
        self.mem_write(STACK + self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    /// Helper function for popping from the stack
    fn stack_pop(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.mem_read(STACK + self.stack_pointer as u16)
    }

    /// Helper function for pushing u16 values to the stack
    fn stack_push_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = data as u8;
        self.stack_push(hi); // Stack grows downwards: lo <- hi <- old
        self.stack_push(lo);
    }

    /// Helper function for popping u16 values from the stack
    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;

        (hi << 8) | lo
    }

    /// JMP with absolute addressing mode
    fn jmp_absolute(&mut self) {
        self.program_counter = self.mem_read_u16(self.program_counter);
    }

    /// JMP with indirect addressing mode
    fn jmp_indirect(&mut self) {
        let pointer = self.mem_read_u16(self.program_counter);

        let lo = self.mem_read(pointer) as u16;
        let hi = if (pointer & 0xFF) == 0xFF {
            self.mem_read(pointer & 0xFF00) as u16
        } else {
            self.mem_read(pointer + 1) as u16
        };

        self.program_counter = (hi << 8) | lo;
    }

    /// JSR (jump to subroutine)
    fn jsr(&mut self, mode: &AddressingMode) {
        self.stack_push_u16(self.program_counter + 2 - 1); // return_addr (after JSR) - 1
        let target = self.get_operand_address(mode); // Absolute value is used directly (not as address)
        self.program_counter = target;
    }

    /// RTS (return from subroutine)
    fn rts(&mut self) {
        let target = self.stack_pop_u16();
        self.program_counter = target + 1;
    }

    /// Generic function for any CPU interrupts
    fn interrupt(&mut self, vector: u16, set_break: bool) {
        // Push PC to stack
        self.stack_push_u16(self.program_counter);

        // Push status to stack, always setting BREAK2 and clearing BREAK
        let mut flags = self.status.bits();
        if set_break {
            flags |= 0b0001_0000;
        } else {
            flags &= 0b1110_1111;
        }
        flags |= 0b0010_0000;
        self.stack_push(flags);

        // Set interrupt disable flag
        self.status.insert(CPUFlags::INTERRUPT_DISABLE);

        // 2 cycles for reading the new PC
        self.bus.tick(2);

        // Set PC from address 0xFFFA
        self.program_counter = self.mem_read_u16(vector);
    }

    /// NMI (non maskable interrupt)
    fn interrupt_nmi(&mut self) {
        self.interrupt(0xFFFA, false);
    }

    /// RTI (return from interrupt)
    fn rti(&mut self) {
        let flags = self.stack_pop();
        self.status = CPUFlags::from_bits_truncate(flags);
        self.status.remove(CPUFlags::BREAK);
        self.status.insert(CPUFlags::BREAK2);

        self.program_counter = self.stack_pop_u16();
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

    /// INC (increment a memory held value)
    fn inc(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let updated_value = value.wrapping_add(1);
        self.mem_write(addr, updated_value);
        self.update_zero_and_negative_flags(updated_value);
    }

    /// DEC (decrement a memory held value)
    fn dec(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let updated_value = value.wrapping_sub(1);
        self.mem_write(addr, updated_value);
        self.update_zero_and_negative_flags(updated_value);
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

    /// ASL (arithmetic shift left) performed on the accumulator
    fn asl_accumulator(&mut self) {
        let shifted = self.register_a << 1;
        self.status
            .set(CPUFlags::CARRY, self.register_a & 0x80 != 0);
        self.set_register_a(shifted);
    }

    /// ASL (arithmetic shift left) performed on a memory held value
    fn asl(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let shifted = value << 1;
        self.status.set(CPUFlags::CARRY, value & 0x80 != 0);
        self.update_zero_and_negative_flags(shifted);
        self.mem_write(addr, shifted);
    }

    /// LSR (logical shift right) performed on the accumulator
    fn lsr_accumulator(&mut self) {
        let shifted = self.register_a >> 1;
        self.status
            .set(CPUFlags::CARRY, self.register_a & 0x01 != 0);
        self.set_register_a(shifted);
    }

    /// LSR (logical shift right) performed on a memory held value
    fn lsr(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let shifted = value >> 1;
        self.status.set(CPUFlags::CARRY, value & 0x01 != 0);
        self.update_zero_and_negative_flags(shifted);
        self.mem_write(addr, shifted);
    }

    /// ROL (rotate left) performed on the accumulator
    fn rol_accumulator(&mut self) {
        let carry = self.status.contains(CPUFlags::CARRY) as u8;
        let rotated = (self.register_a << 1) | carry;
        self.status
            .set(CPUFlags::CARRY, self.register_a & 0x80 != 0);
        self.set_register_a(rotated);
    }

    /// ROL (rotate left) performed on a memory held value
    fn rol(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let carry = self.status.contains(CPUFlags::CARRY) as u8;
        let rotated = (value << 1) | carry;
        self.status.set(CPUFlags::CARRY, value & 0x80 != 0);
        self.update_zero_and_negative_flags(rotated);
        self.mem_write(addr, rotated);
    }

    /// ROR (rotate right) performed on the accumulator
    fn ror_accumulator(&mut self) {
        let carry = self.status.contains(CPUFlags::CARRY) as u8;
        let rotated = (self.register_a >> 1) | (carry << 7);
        self.status
            .set(CPUFlags::CARRY, self.register_a & 0x01 != 0);
        self.set_register_a(rotated);
    }

    /// ROR (rotate right) performed on a memory held value
    fn ror(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let carry = self.status.contains(CPUFlags::CARRY) as u8;
        let rotated = (value >> 1) | (carry << 7);
        self.status.set(CPUFlags::CARRY, value & 0x01 != 0);
        self.update_zero_and_negative_flags(rotated);
        self.mem_write(addr, rotated);
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

    /// CPX (compare register X with memory)
    fn cpx(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let result = self.register_x.wrapping_sub(value);
        self.update_zero_and_negative_flags(result);
        // Set C flag when register X was bigger than value
        self.status.set(CPUFlags::CARRY, self.register_x >= value);
    }

    /// CPY (compare register Y with memory)
    fn cpy(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        let result = self.register_y.wrapping_sub(value);
        self.update_zero_and_negative_flags(result);
        // Set C flag when register Y was bigger than value
        self.status.set(CPUFlags::CARRY, self.register_y >= value);
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
        self.sub_from_register_a(value);
    }

    /// BIT (test if bits are set in memory)
    fn bit(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(mode);
        let value = self.mem_read(addr);

        self.status
            .set(CPUFlags::ZERO, (self.register_a & value) == 0);
        self.status
            .set(CPUFlags::OVERFLOW, (value & 0b0100_0000) != 0);
        self.status
            .set(CPUFlags::NEGATIVE, (value & 0b1000_0000) != 0);
    }

    /// PHA (push register A to stack)
    fn pha(&mut self) {
        self.stack_push(self.register_a);
    }

    /// PLA (push register A from stack)
    fn pla(&mut self) {
        let data = self.stack_pop();
        self.set_register_a(data);
    }

    /// PHP (push processor status to stack)
    fn php(&mut self) {
        // Always set B and unused flag with PHP
        let flags_to_push = self.status.bits() | 0b00110000;
        self.stack_push(flags_to_push);
    }

    /// PLP (pull processor status from stack)
    fn plp(&mut self) {
        let flags = self.stack_pop();
        self.status = CPUFlags::from_bits_truncate(flags);
        self.status.remove(CPUFlags::BREAK);
        self.status.insert(CPUFlags::BREAK2);
    }

    /// TXS (transfer register X to stack pointer)
    fn txs(&mut self) {
        self.stack_pointer = self.register_x;
    }

    /// TSX (transfer stack pointer to register X)
    fn tsx(&mut self) {
        self.set_register_x(self.stack_pointer);
    }

    // SAX (set memory value to result of register A AND register X)
    fn sax(&mut self, mode: &AddressingMode) {
        let addr = self.get_operand_address(&mode);
        self.mem_write(addr, self.register_a & self.register_x);
    }

    pub fn load(&mut self, program: Vec<u8>) {
        for i in 0..(program.len() as u16) {
            self.bus.write_prg_rom(0x8000 + i, program[i as usize]);
        }
        self.bus.set_reset_vector(0x8000);
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.status = CPUFlags::from_bits_truncate(0b00100100);

        self.stack_pointer = STACK_RESET;
        self.program_counter = self.mem_read_u16(0xFFFC);
        self.bus.tick(7);
    }

    pub fn run(&mut self) {
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut CPU),
    {
        let opcodes = &*opcodes::OPCODES_MAP;

        loop {
            if self.bus.poll_nmi_status() {
                self.interrupt_nmi();
            }

            callback(self);

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

                // ASL (arithmetic shift left)
                // performed on the accumulator
                0x0a => self.asl_accumulator(),
                // performed on a memory held value
                0x06 | 0x16 | 0x0e | 0x1e => {
                    self.asl(&opcode.mode);
                }

                // LSR (logical shift right)
                // performed on the accumulator
                0x4a => self.lsr_accumulator(),
                // performed on a memory held value
                0x46 | 0x56 | 0x4e | 0x5e => {
                    self.lsr(&opcode.mode);
                }

                // ROL (rotate left)
                // performed on the accumulator
                0x2a => self.rol_accumulator(),
                // performed on a memory held value
                0x26 | 0x36 | 0x2e | 0x3e => {
                    self.rol(&opcode.mode);
                }

                // ROR (rotate right)
                // performed on the accumulator
                0x6a => self.ror_accumulator(),
                // performed on a memory held value
                0x66 | 0x76 | 0x6e | 0x7e => {
                    self.ror(&opcode.mode);
                }

                // CMP (compare register A with memory)
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => {
                    self.cmp(&opcode.mode);
                }

                // CPX (compare register X with memory)
                0xe0 | 0xe4 | 0xec => {
                    self.cpx(&opcode.mode);
                }

                // CPY (compare register Y with memory)
                0xc0 | 0xc4 | 0xcc => {
                    self.cpy(&opcode.mode);
                }

                // ADC (add to register A with carry-in)
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => {
                    self.adc(&opcode.mode);
                }

                // SBC (substract from register A with borrow-in); 0xeb is an illegal alias for 0xe9
                0xeb | 0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => {
                    self.sbc(&opcode.mode);
                }

                // BIT (test if bits are set in memory)
                0x24 | 0x2c => {
                    self.bit(&opcode.mode);
                }

                // INC (increment a memory held value)
                0xe6 | 0xf6 | 0xee | 0xfe => {
                    self.inc(&opcode.mode);
                }

                // DEC (decrement a memory held value)
                0xc6 | 0xd6 | 0xce | 0xde => {
                    self.dec(&opcode.mode);
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
                0xf0 => self.branch(self.status.contains(CPUFlags::ZERO)), // BEQ (branch if zero flag is set)
                0xd0 => self.branch(!self.status.contains(CPUFlags::ZERO)), // BNE (branch if zero flag is clear)
                0x10 => self.branch(!self.status.contains(CPUFlags::NEGATIVE)), // BPL
                0x30 => self.branch(self.status.contains(CPUFlags::NEGATIVE)), // BMI
                0x50 => self.branch(!self.status.contains(CPUFlags::OVERFLOW)), // BVC
                0x70 => self.branch(self.status.contains(CPUFlags::OVERFLOW)), // BVS
                0x90 => self.branch(!self.status.contains(CPUFlags::CARRY)), // BCC
                0xb0 => self.branch(self.status.contains(CPUFlags::CARRY)), // BCS
                0x4c => self.jmp_absolute(), // JMP with absolute addressing mode
                0x6c => self.jmp_indirect(), // JMP with indirect addressing mode
                0x20 => self.jsr(&opcode.mode), // JSR (jump to subroutine)
                0x60 => self.rts(),          // RTS (return from subroutine)
                0x48 => self.pha(),          // PHA (push register A to stack)
                0x68 => self.pla(),          // PLA (pull register A from stack)
                0x08 => self.php(),          // PHP (push processor status to stack)
                0x28 => self.plp(),          // PLP (pull processor status from stack)
                0x9a => self.txs(),          // TXS (transfer register X to stack pointer)
                0xba => self.tsx(),          // TSX (transfer stack pointer to register X)
                0x40 => self.rti(),          // RTI (return from interrupt)
                0xea => {}                   // NOP (do nothing, only increment program counter)

                // BRK
                0x00 => {
                    if self.mem_read_u16(0xFFFE) == 0x0000 {
                        return;
                    }
                    self.program_counter += 1;
                    self.interrupt(0xFFFE, true);
                }

                // UNOFFICIAL OPCODES
                // NOP (do nothing, but read a value)
                0x04 | 0x44 | 0x64 | 0x0c | 0x14 | 0x34 | 0x54 | 0x74 | 0xd4 | 0xf4 | 0x80
                | 0x1c | 0x3c | 0x5c | 0x7c | 0xdc | 0xfc => {
                    let addr = self.get_operand_address(&opcode.mode);
                    let _data = self.mem_read(addr);
                }
                // NOP (do nothing)
                0x1a | 0x3a | 0x5a | 0x7a | 0xda | 0xfa => { /* do nothing */ }
                // LAX (load value to register A and to register X)
                0xa3 | 0xb3 | 0xa7 | 0xb7 | 0xaf | 0xbf => {
                    self.lda(&opcode.mode);
                    self.tax();
                }
                // SAX (set memory value to result of register A AND register X)
                0x87 | 0x97 | 0x83 | 0x8f => self.sax(&opcode.mode),
                // DCP (decrement a memory held value and compare it with register A)
                0xc7 | 0xd7 | 0xc3 | 0xd3 | 0xcf | 0xdf | 0xdb => {
                    self.dec(&opcode.mode);
                    self.cmp(&opcode.mode);
                }
                // ISB (increment a memory held value and substract it from register A)
                0xe7 | 0xf7 | 0xe3 | 0xf3 | 0xef | 0xff | 0xfb => {
                    self.inc(&opcode.mode);
                    self.sbc(&opcode.mode);
                }
                // SLO (shift left and perform OR with the result on register A)
                0x07 | 0x17 | 0x03 | 0x13 | 0x0f | 0x1f | 0x1b => {
                    self.asl(&opcode.mode);
                    self.ora(&opcode.mode);
                }
                // RLA (rotate left and perform AND with the result on register A)
                0x27 | 0x37 | 0x23 | 0x33 | 0x2f | 0x3f | 0x3b => {
                    self.rol(&opcode.mode);
                    self.and(&opcode.mode);
                }
                // SRE (shift right and perform XOR with the result on register A)
                0x47 | 0x57 | 0x43 | 0x53 | 0x4f | 0x5f | 0x5b => {
                    self.lsr(&opcode.mode);
                    self.eor(&opcode.mode);
                }
                // RRA (rotate right and add the result to register A)
                0x67 | 0x77 | 0x63 | 0x73 | 0x6f | 0x7f | 0x7b => {
                    self.ror(&opcode.mode);
                    self.adc(&opcode.mode);
                }
                _ => unimplemented!(
                    "opcode 0x{:02x} ({}) has no dispatch arm in run()",
                    code,
                    opcode.mnemonic
                ), // A not implemented operation code
            }

            if self.page_crossed && opcode.page_cross {
                self.cycle_debt += 1;
            }
            self.bus.tick(opcode.cycles as u16 + self.cycle_debt as u16);
            self.cycle_debt = 0;
            self.page_crossed = false;

            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cartridge::test;

    // ----- Memory primitives (Mem trait) -----

    #[test]
    fn test_mem_read_write() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        cpu.mem_write(0x1234, 0x42);
        assert_eq!(cpu.mem_read(0x1234), 0x42);
    }

    #[test]
    fn test_mem_read_write_u16_little_endian() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        cpu.mem_write_u16(0x1234, 0xBEEF);
        assert_eq!(cpu.mem_read(0x1234), 0xEF);
        assert_eq!(cpu.mem_read(0x1235), 0xBE);
        assert_eq!(cpu.mem_read_u16(0x1234), 0xBEEF);
    }

    // ----- CPU lifecycle -----

    #[test]
    fn test_reset_clears_registers_and_loads_pc() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        cpu.register_a = 0xFF;
        cpu.register_x = 0xFF;
        cpu.register_y = 0xFF;
        cpu.status = CPUFlags::from_bits_truncate(0xFF);
        cpu.bus.set_reset_vector(0x8000);

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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        cpu.load(vec![0xa9, 0x42, 0x00]);

        assert_eq!(cpu.mem_read(0x8000), 0xa9);
        assert_eq!(cpu.mem_read(0x8001), 0x42);
        assert_eq!(cpu.mem_read(0x8002), 0x00);
        assert_eq!(cpu.mem_read_u16(0xFFFC), 0x8000);
    }

    // ----- LDA addressing modes -----

    #[test]
    fn test_lda_immediate() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x42, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_zero_page() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa5, 0x10, 0x00])));
        cpu.mem_write(0x10, 0x42);
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_zero_page_x_with_wrap() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x02, 0xaa, 0xb5, 0xff, 0x00,
        ])));
        cpu.mem_write(0x01, 0x42);
        // LDA #$02; TAX; LDA $FF,X  -> 0xFF + 2 wraps to 0x01
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_absolute() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xad, 0x34, 0x12, 0x00])));
        cpu.mem_write(0x1234, 0x42);
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_absolute_x() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x02, 0xaa, 0xbd, 0x34, 0x12, 0x00,
        ])));
        cpu.mem_write(0x1236, 0x42);
        // LDA #$02; TAX; LDA $1234,X
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_absolute_y() {
        // No LDY/TAY instruction exists yet, so set register_y manually after reset.
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xb9, 0x34, 0x12, 0x00])));
        cpu.mem_write(0x1236, 0x42);
        cpu.register_y = 0x02;
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_indirect_x_with_wrap() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x02, 0xaa, 0xa1, 0xfe, 0x00,
        ])));
        // Pointer at zero-page 0x00/0x01 -> effective address 0x1234
        cpu.mem_write(0x00, 0x34);
        cpu.mem_write(0x01, 0x12);
        cpu.mem_write(0x1234, 0x42);
        // LDA #$02; TAX; LDA ($FE,X)  -> 0xFE + 2 wraps to 0x00
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_lda_indirect_y() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xb1, 0x10, 0x00])));
        // Pointer at zero-page 0x10/0x11 -> base 0x1234, +Y(2) = 0x1236
        cpu.mem_write(0x10, 0x34);
        cpu.mem_write(0x11, 0x12);
        cpu.mem_write(0x1236, 0x42);
        cpu.register_y = 0x02;
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    // ----- Zero & Negative flags (tested via LDA) -----

    #[test]
    fn test_lda_sets_zero_flag_when_zero() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x00, 0x00])));
        cpu.run();
        assert!(cpu.status.contains(CPUFlags::ZERO), "Z flag should be set");
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag should be clear"
        );
    }

    #[test]
    fn test_lda_sets_negative_flag_when_high_bit_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x80, 0x00])));
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x42, 0x85, 0x10, 0x00])));
        // LDA #$42; STA $10
        cpu.run();
        assert_eq!(cpu.mem_read(0x10), 0x42);
    }

    // ----- LDX -----

    #[test]
    fn test_ldx_immediate() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa2, 0x42, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_x, 0x42);
    }

    #[test]
    fn test_ldx_zero_page_y_with_wrap() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x02, 0xa8, 0xb6, 0xff, 0x00,
        ])));
        cpu.mem_write(0x01, 0x42);
        // LDA #$02; TAY; LDX $FF,Y  -> 0xFF + 2 wraps to 0x01
        cpu.run();
        assert_eq!(cpu.register_x, 0x42);
    }

    // ----- LDY -----

    #[test]
    fn test_ldy_immediate() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa0, 0x42, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_y, 0x42);
    }

    // ----- STX -----

    #[test]
    fn test_stx_writes_x_to_memory() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x42, 0xaa, 0x86, 0x10, 0x00,
        ])));
        // LDA #$42; TAX; STX $10; BRK
        // TAX is needed because X starts at 0 after reset — without it we could
        // not distinguish a working STX from a broken one that writes 0.
        cpu.run();
        assert_eq!(cpu.mem_read(0x10), 0x42);
    }

    // ----- STY -----

    #[test]
    fn test_sty_writes_y_to_memory() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x42, 0xa8, 0x84, 0x10, 0x00,
        ])));
        // LDA #$42; TAY; STY $10; BRK
        // TAY is needed because Y starts at 0 after reset — without it we could
        // not distinguish a working STY from a broken one that writes 0.
        cpu.run();
        assert_eq!(cpu.mem_read(0x10), 0x42);
    }

    // ----- CLC & SEC (Carry flag) -----

    #[test]
    fn test_sec_sets_carry_flag() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0x00])));
        // C starts at 0 after reset; SEC must set bit 0 of the status register.
        cpu.run();
        assert!(cpu.status.contains(CPUFlags::CARRY), "C flag should be set");
    }

    #[test]
    fn test_clc_clears_carry_flag() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0x18, 0x00])));
        // SEC; CLC; BRK -- C is set then cleared, exercising both transitions.
        cpu.run();
        assert!(
            !cpu.status.contains(CPUFlags::CARRY),
            "C flag should be clear"
        );
    }

    // ----- AND -----

    #[test]
    fn test_and_immediate() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0xaa, 0x29, 0x55, 0x00])));
        // LDA #$AA; AND #$55 -> 0xAA & 0x55 = 0x00, Z flag must be set.
        // This single test exercises: opcode wiring, the AND operation, storing
        // the result back into A, and Z-flag handling (a buggy AND that forgot
        // to call update_zero_and_negative_flags would fail the Z assertion).
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x05, 0xc9, 0x05, 0x00])));
        // LDA #$05; CMP #$05 -> A - M = 0, so Z=1, C=1 (A >= M).
        // Register A must be unchanged (CMP is a side-effect-free comparison).
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x38, 0xa9, 0x00, 0xc9, 0x01, 0x00,
        ])));
        // SEC first so C starts at 1: a buggy CMP that fails to update C
        // (or sets it backwards) would fail the final C==0 assertion.
        // Then LDA #$00; CMP #$01 -> 0 - 1 = 0xFF (borrow), so C=0, Z=0, N=1.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x05, 0x69, 0x03, 0x00])));
        // C starts at 0 after reset. 5 + 3 = 8, no overflow anywhere.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x38, 0xa9, 0x05, 0x69, 0x03, 0x00,
        ])));
        // SEC; LDA #$05; ADC #$03 -> 5 + 3 + 1 = 9.
        // A buggy ADC that ignored the carry-in would produce 8, not 9.
        cpu.run();
        assert_eq!(cpu.register_a, 0x09, "carry-in must be added");
        assert!(!cpu.status.contains(CPUFlags::CARRY), "C should be clear");
        assert!(
            !cpu.status.contains(CPUFlags::OVERFLOW),
            "V should be clear"
        );
    }

    #[test]
    fn test_adc_carry_out_sets_c_and_zero_result_sets_z() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0xff, 0x69, 0x01, 0x00])));
        // LDA #$FF; ADC #$01 -> 0xFF + 0x01 = 0x100, so A=0x00, C=1, Z=1.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x50, 0x69, 0x50, 0x00])));
        // LDA #$50; ADC #$50 -> 0x50 + 0x50 = 0xA0.
        // In signed: (+80) + (+80) = (-96) is an overflow, so V must be set.
        // 0xA0 also has bit 7 set, so N must be set.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x38, 0xa9, 0x05, 0xe9, 0x03, 0x00,
        ])));
        // SEC; LDA #$05; SBC #$03 -> 5 - 3 - 0 = 2, no borrow anywhere.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x18, 0xa9, 0x05, 0xe9, 0x03, 0x00,
        ])));
        // CLC; LDA #$05; SBC #$03 -> 5 - 3 - 1 = 1.
        // A buggy SBC that ignored the carry-in would produce 2, not 1.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x38, 0xa9, 0x00, 0xe9, 0x01, 0x00,
        ])));
        // SEC; LDA #$00; SBC #$01 -> 0 - 1 - 0 = 0xFF (borrow occurred).
        // V should be clear: 0 - 1 = -1, which is valid in signed 8-bit.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x38, 0xa9, 0x80, 0xe9, 0x01, 0x00,
        ])));
        // SEC; LDA #$80; SBC #$01 -> 0x80 - 0x01 - 0 = 0x7F.
        // In signed: (-128) - (+1) = -129, which doesn't fit in 8-bit signed,
        // so signed overflow occurred and V must be set.
        // 0x7F has bit 7 clear, so N must be clear.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0xaa, 0x09, 0x55, 0x00])));
        // LDA #$AA; ORA #$55 -> 0xAA | 0x55 = 0xFF, N flag set, Z flag clear.
        // Like AND, this is a direct-mirror pattern: read operand from memory
        // via get_operand_address, OR with A, update flags.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0xaa, 0x49, 0xfe, 0x00])));
        // LDA #$AA; EOR #$FE -> 0xAA ^ 0xFE = 0x54.
        // Picked so the result differs from OR (0xFE) and AND (0xAA),
        // proving XOR is wired and not accidentally aliased to another op.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xf8, 0x00])));
        cpu.run();
        assert!(
            cpu.status.contains(CPUFlags::DECIMAL_MODE),
            "D flag should be set"
        );
    }

    #[test]
    fn test_cld_clears_decimal_flag() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xf8, 0xd8, 0x00])));
        // SED; CLD — exercises both the set and the clear transition.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x78, 0x00])));
        cpu.run();
        assert!(
            cpu.status.contains(CPUFlags::INTERRUPT_DISABLE),
            "I flag should be set"
        );
    }

    #[test]
    fn test_cli_clears_interrupt_flag() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x78, 0x58, 0x00])));
        // SEI; CLI — exercises both transitions.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x50, 0x69, 0x50, 0xb8, 0x00,
        ])));
        // LDA #$50; ADC #$50 -> V=1 (signed overflow), then CLV -> V=0.
        // N and Z should be unchanged by CLV (proving it's not a sledgehammer
        // that clears the whole status register).
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x42, 0xaa, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_x, 0x42);
    }

    #[test]
    fn test_tay_transfers_a_to_y() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x42, 0xa8, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_y, 0x42);
    }

    #[test]
    fn test_txa_transfers_x_to_a() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x42, 0xaa, 0xa9, 0x00, 0x8a, 0x00,
        ])));
        // LDA #$42; TAX; LDA #$00; TXA; BRK
        // A is overwritten between TAX and TXA so the assertion proves X -> A actually happened.
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_tya_transfers_y_to_a() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x42, 0xa8, 0xa9, 0x00, 0x98, 0x00,
        ])));
        // LDA #$42; TAY; LDA #$00; TYA; BRK
        // A is overwritten between TAY and TYA so the assertion proves Y -> A actually happened.
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    #[test]
    fn test_inx_increments_x() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x05, 0xaa, 0xe8, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_x, 0x06);
    }

    #[test]
    fn test_inx_overflow_wraps_to_zero() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0xff, 0xaa, 0xe8, 0x00])));
        cpu.run();
        assert_eq!(cpu.register_x, 0x00);
        assert!(
            cpu.status.contains(CPUFlags::ZERO),
            "Z flag should be set on wrap"
        );
    }

    #[test]
    fn test_iny_increments_y() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x05, 0xa8, 0xc8, 0x00])));
        // LDA #$05; TAY; INY; BRK
        cpu.run();
        assert_eq!(cpu.register_y, 0x06);
    }

    #[test]
    fn test_dex_underflow_wraps_to_0xff() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xca, 0x00])));
        // X starts at 0 after reset; DEX -> 0xFF (underflow), sets N flag.
        cpu.run();
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
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x05, 0xa8, 0x88, 0x00])));
        // LDA #$05; TAY; DEY; BRK
        cpu.run();
        assert_eq!(cpu.register_y, 0x04);
    }

    // ----- NOP -----

    #[test]
    fn test_nop_does_nothing() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x42, 0xea, 0x00])));
        // LDA #$42; NOP; BRK -- NOP must not modify A
        cpu.run();
        assert_eq!(cpu.register_a, 0x42);
    }

    // ----- CPX & CPY (compare X / compare Y) -----

    #[test]
    fn test_cpx_equal_sets_z_and_c() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa2, 0x42, 0xe0, 0x42, 0x00])));
        // LDX #$42; CPX #$42 -> equal: Z=1, C=1. X must not be modified.
        cpu.run();
        assert_eq!(cpu.register_x, 0x42, "CPX must not modify X");
        assert!(cpu.status.contains(CPUFlags::ZERO), "Z should be set");
        assert!(cpu.status.contains(CPUFlags::CARRY), "C should be set");
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be clear"
        );
    }

    #[test]
    fn test_cpy_equal_sets_z_and_c() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa0, 0x42, 0xc0, 0x42, 0x00])));
        // LDY #$42; CPY #$42 -> equal: Z=1, C=1. Y must not be modified.
        cpu.run();
        assert_eq!(cpu.register_y, 0x42, "CPY must not modify Y");
        assert!(cpu.status.contains(CPUFlags::ZERO), "Z should be set");
        assert!(cpu.status.contains(CPUFlags::CARRY), "C should be set");
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "N should be clear"
        );
    }

    // ----- INC & DEC (increment/decrement memory) -----

    // First instructions that read, modify, AND write back to the same
    // memory address. All existing instructions either read into a register
    // or write from a register — INC/DEC do both.

    #[test]
    fn test_inc_increments_memory() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xe6, 0x10, 0x00])));
        cpu.mem_write(0x10, 0x01);
        cpu.run();
        assert_eq!(cpu.mem_read(0x10), 0x02);
    }

    #[test]
    fn test_dec_decrements_memory() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xc6, 0x10, 0x00])));
        cpu.mem_write(0x10, 0x02);
        cpu.run();
        assert_eq!(cpu.mem_read(0x10), 0x01);
    }

    // ----- BEQ (branch if equal, i.e. Z=1) -----

    // Branches use Relative addressing: a signed offset byte is read from
    // the program stream. If the condition is met, the offset is added to PC;
    // otherwise execution falls through to the next instruction.
    //
    // The offset is relative to the byte AFTER the branch instruction
    // (i.e. PC already points past the opcode when the offset is read).

    #[test]
    fn test_beq_branches_when_zero_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x00, 0xf0, 0x02, 0x00])));
        // LDA #$00 -> sets Z=1, then BEQ +2 skips the BRK at 0x8004 and
        // lands on zeroed memory (0x00 = BRK) at 0x8006.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8007);
    }

    #[test]
    fn test_beq_does_not_branch_when_zero_clear() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xf0, 0x02, 0x00])));
        // Z=0 after reset, so BEQ falls through to BRK at 0x8002.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8003);
    }

    // ----- BNE (branch if not equal, i.e. Z=0) -----

    #[test]
    fn test_bne_branches_when_zero_clear() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xd0, 0x02, 0x00])));
        // Z=0 after reset, so BNE branches over the BRK to zeroed memory (BRK).
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_bne_does_not_branch_when_zero_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x00, 0xd0, 0x02, 0x00])));
        // LDA #$00 sets Z=1, so BNE falls through to BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    // ----- BPL & BMI (sign-based branches) -----

    // BPL ($10): branch if N=0. BMI ($30): branch if N=1.

    #[test]
    fn test_bpl_branches_when_positive() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x10, 0x02, 0x00])));
        // N=0 after reset, BPL takes the branch over BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_bpl_does_not_branch_when_negative() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x80, 0x10, 0x02, 0x00])));
        // LDA #$80 sets N=1, BPL falls through.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_bmi_branches_when_negative() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x80, 0x30, 0x02, 0x00])));
        // LDA #$80 sets N=1, BMI branches over BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8007);
    }

    #[test]
    fn test_bmi_does_not_branch_when_positive() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x30, 0x02, 0x00])));
        // N=0 after reset, BMI falls through.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8003);
    }

    // ----- BCC & BCS (carry-based branches) -----

    // BCC ($90): branch if C=0. BCS ($B0): branch if C=1.

    #[test]
    fn test_bcc_branches_when_carry_clear() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x90, 0x02, 0x00])));
        // C=0 after reset, BCC takes the branch over BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_bcc_does_not_branch_when_carry_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0x90, 0x02, 0x00])));
        // SEC sets C=1, BCC falls through.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8004);
    }

    #[test]
    fn test_bcs_branches_when_carry_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0xb0, 0x02, 0x00])));
        // SEC sets C=1, BCS branches over BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8006);
    }

    #[test]
    fn test_bcs_does_not_branch_when_carry_clear() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xb0, 0x02, 0x00])));
        // C=0 after reset, BCS falls through.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8003);
    }

    // ----- BVC & BVS (overflow-based branches) -----

    // BVC ($50): branch if V=0. BVS ($70): branch if V=1.

    #[test]
    fn test_bvc_branches_when_overflow_clear() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x50, 0x02, 0x00])));
        // V=0 after reset, BVC takes the branch over BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_bvc_does_not_branch_when_overflow_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x50, 0x69, 0x50, 0x50, 0x02, 0x00,
        ])));
        // LDA #$50; ADC #$50 sets V=1, BVC falls through.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8007);
    }

    #[test]
    fn test_bvs_branches_when_overflow_set() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x50, 0x69, 0x50, 0x70, 0x02, 0x00,
        ])));
        // LDA #$50; ADC #$50 sets V=1, BVS branches over BRK.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8009);
    }

    #[test]
    fn test_bvs_does_not_branch_when_overflow_clear() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x70, 0x02, 0x00])));
        // V=0 after reset, BVS falls through.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8003);
    }

    // ----- BIT (Bit Test) -----

    // BIT ($24/$2C) sets N = bit 7 of memory, V = bit 6 of memory,
    // Z = (A & memory) == 0, and does NOT modify A. It uses ZeroPage
    // and Absolute modes (both covered by LDA tests), so one functional
    // test suffices.

    #[test]
    fn test_bit_sets_n_v_z_flags_from_memory() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x03, 0x24, 0x10, 0x00])));
        // Pre-seed $10 with 0xC0 (bit 7 = 1, bit 6 = 1).
        cpu.mem_write(0x0010, 0xC0);
        // LDA #$03; BIT $10; BRK
        cpu.run();
        assert_eq!(cpu.register_a, 0x03, "A must not be modified by BIT");
        assert!(
            cpu.status.contains(CPUFlags::NEGATIVE),
            "N flag: bit 7 of 0xC0 is set"
        );
        assert!(
            cpu.status.contains(CPUFlags::OVERFLOW),
            "V flag: bit 6 of 0xC0 is set"
        );
        assert!(
            cpu.status.contains(CPUFlags::ZERO),
            "Z flag: 0x03 & 0xC0 == 0, so Z must be set"
        );
    }

    // ----- JMP (Jump) -----

    // JMP Absolute ($4C): set PC to the address in the next two bytes.
    // JMP Indirect ($6C): set PC to the address stored at the pointer
    // in the next two bytes. The NMOS 6502 has a page-wrap bug: when
    // the pointer address ends in $FF, the high byte is fetched from
    // $xx00 instead of $(xx+1)$00.

    #[test]
    fn test_jmp_absolute_jumps_to_address() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x4c, 0x04, 0x80, 0x00])));
        // JMP $8004 skips the fall-through BRK at $8003 and lands at
        // $8004 (zeroed memory = BRK). Final PC = $8005.
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_jmp_indirect_jumps_through_pointer() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x6c, 0x10, 0x00, 0x00])));
        // Pre-write pointer value $8004 at $0010-$0011.
        cpu.mem_write(0x0010, 0x04);
        cpu.mem_write(0x0011, 0x80);
        // JMP ($0010); BRK
        cpu.run();
        assert_eq!(cpu.program_counter, 0x8005);
    }

    #[test]
    fn test_jmp_indirect_page_wrap_bug() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x6c, 0xff, 0x01, 0x00])));
        // Pre-write pointer at $01FF: low=$08, high=$80.
        // The NMOS bug means high is read from $0100 (not $0200),
        // giving target $8008 instead of $0008.
        cpu.mem_write(0x01FF, 0x08);
        cpu.mem_write(0x0100, 0x80);
        // JMP ($01FF); BRK
        cpu.run();
        // With bug: target = $8008 → BRK at $8008 → PC = $8009
        assert_eq!(cpu.program_counter, 0x8009);
    }

    // ----- PHA & PLA (stack push/pop for A) -----

    // PHA ($48): push A onto stack (pre-decrement store).
    // PLA ($68): pull from stack into A (post-increment load), sets N/Z.

    #[test]
    fn test_pha_pushes_a_onto_stack_and_decrements_sp() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x42, 0x48, 0x00])));
        // LDA #$42; PHA; BRK
        cpu.run();
        assert_eq!(
            cpu.stack_pointer, 0xFC,
            "SP decremented once from initial 0xFD"
        );
        assert_eq!(
            cpu.mem_read(0x01FD),
            0x42,
            "A=0x42 stored at current SP before decrement (0x0100|initial_SP)"
        );
    }

    #[test]
    fn test_pla_round_trip_restores_a_and_sp() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa9, 0x42, 0x48, 0xa9, 0x00, 0x68, 0x00,
        ])));
        // LDA #$42; PHA; LDA #$00; PLA; BRK
        cpu.run();
        assert_eq!(cpu.register_a, 0x42, "A should be restored from stack");
        assert_eq!(
            cpu.stack_pointer, 0xFD,
            "SP should return to initial 0xFD after push/pop"
        );
    }

    // ----- PHP & PLP (stack push/pop for status) -----

    // PHP ($08): push status onto stack.
    // PLP ($28): pull from stack into status, restoring all flags.

    #[test]
    fn test_php_plp_round_trip_restores_status() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0x08, 0x18, 0x28, 0x00])));
        // SEC (C=1); PHP; CLC (C=0); PLP; BRK
        cpu.run();
        assert!(
            cpu.status.contains(CPUFlags::CARRY),
            "C should be 1 after PLP restores the status pushed after SEC"
        );
    }

    // ----- TXS & TSX (stack pointer transfers) -----

    // TXS ($9A): transfer X to SP (no flags).
    // TSX ($BA): transfer SP to X (sets N/Z from X).

    #[test]
    fn test_txs_transfers_x_to_stack_pointer() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa2, 0x42, 0x9a, 0x00])));
        // LDX #$42; TXS; BRK
        cpu.run();
        assert_eq!(cpu.stack_pointer, 0x42, "SP should equal X after TXS");
    }

    #[test]
    fn test_tsx_transfers_stack_pointer_to_x() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0xa2, 0x42, 0x9a, 0xa2, 0x00, 0xba, 0x00,
        ])));
        // LDX #$42; TXS (SP=0x42); LDX #$00 (clobber X); TSX (X=SP); BRK
        cpu.run();
        assert_eq!(cpu.register_x, 0x42, "X should be restored from SP");
    }

    // ----- JSR & RTS (subroutine call/return) -----

    // JSR ($20): push return address minus 1 onto stack, jump to target.
    // RTS ($60): pop return address, add 1, and jump there.

    #[test]
    fn test_jsr_rts_subroutine_call_and_return() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![
            0x20, 0x07, 0x80, // JSR $8007
            0xa9, 0x42, // LDA #$42 (return here)
            0x00, 0x00, // BRK as padding
            0xa9, 0x03, // LDA #$03 (subroutine)
            0x60, // RTS
        ])));
        // 0x8000: JSR $8007     -> call subroutine
        // 0x8003: LDA #$42      -> return here after RTS
        // 0x8005: BRK
        // 0x8005-0x8006: padding
        // 0x8007: LDA #$03      -> subroutine clobbers A
        // 0x8009: RTS
        cpu.run();
        assert_eq!(
            cpu.register_a, 0x42,
            "A should be 0x42 after returning from subroutine"
        );
        assert_eq!(cpu.stack_pointer, 0xFD, "SP should return to initial 0xFD");
    }

    // ----- ASL, LSR, ROL, ROR (shifts & rotates) -----

    // ASL (0x0A): arithmetic shift left, C = bit 7, result <<= 1
    #[test]
    fn test_asl_accumulator_shifts_left_and_sets_carry() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x80, 0x0a, 0x00])));
        // LDA #$80; ASL A; BRK
        cpu.run();
        assert_eq!(cpu.register_a, 0x00, "$80 << 1 = $00 (truncated)");
        assert!(cpu.status.contains(CPUFlags::CARRY), "C = bit 7 of $80");
        assert!(cpu.status.contains(CPUFlags::ZERO), "result is 0");
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "bit 7 of result is 0"
        );
    }

    // LSR (0x4A): logical shift right, C = bit 0, result >>= 1
    #[test]
    fn test_lsr_accumulator_shifts_right_and_sets_carry() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0x01, 0x4a, 0x00])));
        // LDA #$01; LSR A; BRK
        cpu.run();
        assert_eq!(cpu.register_a, 0x00, "$01 >> 1 = $00");
        assert!(cpu.status.contains(CPUFlags::CARRY), "C = bit 0 of $01");
        assert!(cpu.status.contains(CPUFlags::ZERO), "result is 0");
        assert!(
            !cpu.status.contains(CPUFlags::NEGATIVE),
            "bit 7 of result is 0"
        );
    }

    // ROL (0x2A): rotate left through carry, C → bit 0, bit 7 → C
    #[test]
    fn test_rol_accumulator_rotates_left_through_carry() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0xa9, 0x80, 0x2a, 0x00])));
        // SEC; LDA #$80; ROL A; BRK
        cpu.run();
        // A = ($80 << 1) | 1 (carry-in) = $01
        // C = bit 7 of $80 = 1
        assert_eq!(cpu.register_a, 0x01);
        assert!(cpu.status.contains(CPUFlags::CARRY));
        assert!(!cpu.status.contains(CPUFlags::ZERO));
        assert!(!cpu.status.contains(CPUFlags::NEGATIVE));
    }

    // ROR (0x6A): rotate right through carry, C → bit 7, bit 0 → C
    #[test]
    fn test_ror_accumulator_rotates_right_through_carry() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x38, 0xa9, 0x01, 0x6a, 0x00])));
        // SEC; LDA #$01; ROR A; BRK
        cpu.run();
        // A = ($01 >> 1) | ($80 carry-in) = $80
        // C = bit 0 of $01 = 1
        assert_eq!(cpu.register_a, 0x80);
        assert!(cpu.status.contains(CPUFlags::CARRY));
        assert!(!cpu.status.contains(CPUFlags::ZERO));
        assert!(cpu.status.contains(CPUFlags::NEGATIVE));
    }

    // ----- RTI (Return from Interrupt) -----

    // RTI ($40): pop status then PC (16-bit, no +1) from stack.
    // Equivalent to PLP followed by a RTS that doesn't add 1.

    #[test]
    fn test_rti_restores_status_and_pc_from_stack() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa2, 0xf9, 0x9a, 0x40])));
        // Simulate stack state after a BRK/IRQ would have pushed it:
        //   push PC high | push PC low | push status
        // We manually pre-write the stack and use TXS to set SP.
        cpu.mem_write(0x01FA, 0b00100101); // status with C=1
        cpu.mem_write(0x01FB, 0x04); // PC low = $04
        cpu.mem_write(0x01FC, 0x80); // PC high = $80
        // LDX #$F9; TXS (SP=$F9); RTI
        // After RTI: SP=$FC, PC=$8004, then BRK at $8004 halts
        cpu.run();
        assert_eq!(
            cpu.program_counter, 0x8005,
            "RTI returns to $8004 (BRK), which halts at $8005"
        );
        assert_eq!(cpu.stack_pointer, 0xFC, "SP after 3 pops from $F9");
        assert!(
            cpu.status.contains(CPUFlags::CARRY),
            "C flag restored from pushed status"
        );
    }

    // ----- NMI Interrupt -----

    #[test]
    fn test_cpu_interrupt_nmi_pushes_pc_and_status() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        cpu.program_counter = 0xBEEF;
        cpu.interrupt_nmi();
        // SP decremented by 3 (2 for PC + 1 for status)
        assert_eq!(cpu.stack_pointer, 0xFA);
        // Status byte pushed at 0x01FB (with BREAK=0, BREAK2=1)
        let pushed_status = cpu.mem_read(0x01FB);
        assert_eq!(pushed_status & 0b00010000, 0, "BREAK flag should be 0");
        assert_ne!(pushed_status & 0b00100000, 0, "BREAK2 flag should be 1");
        // PC = 0xBEEF at 0x01FC (low) and 0x01FD (high)
        assert_eq!(cpu.mem_read(0x01FC), 0xEF, "PC low byte");
        assert_eq!(cpu.mem_read(0x01FD), 0xBE, "PC high byte");
    }

    #[test]
    fn test_cpu_interrupt_nmi_loads_vector() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        // Write NMI vector at 0xFFFA
        cpu.bus.write_prg_rom(0xFFFA, 0x34);
        cpu.bus.write_prg_rom(0xFFFB, 0x12);
        cpu.interrupt_nmi();
        assert_eq!(cpu.program_counter, 0x1234);
    }

    #[test]
    fn test_cpu_interrupt_nmi_sets_interrupt_disable() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0x00])));
        cpu.status.remove(CPUFlags::INTERRUPT_DISABLE);
        assert!(!cpu.status.contains(CPUFlags::INTERRUPT_DISABLE));
        cpu.interrupt_nmi();
        assert!(cpu.status.contains(CPUFlags::INTERRUPT_DISABLE));
    }

    // ----- Integration -----

    #[test]
    fn test_lda_tax_inx_program() {
        let mut cpu = CPU::new(Bus::new(test::test_rom(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00])));
        // LDA #$c0; TAX; INX; BRK
        cpu.run();
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
