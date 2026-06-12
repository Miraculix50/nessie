use crate::cpu::AddressingMode;
use std::collections::HashMap;
use std::sync::LazyLock;

pub struct OpCode {
    pub code: u8,
    pub mnemonic: &'static str,
    pub len: u8,
    pub cycles: u8,
    pub mode: AddressingMode,
}

impl OpCode {
    pub const fn new(
        code: u8,
        mnemonic: &'static str,
        len: u8,
        cycles: u8,
        mode: AddressingMode,
    ) -> Self {
        OpCode {
            code,
            mnemonic,
            len,
            cycles,
            mode,
        }
    }
}

pub static CPU_OPCODES: &[OpCode] = &[
    // BRK (stop program execution)
    OpCode::new(0x00, "BRK", 1, 7, AddressingMode::NoneAddressing),
    // NOP (do nothing, only increment program counter)
    OpCode::new(0xea, "NOP", 1, 2, AddressingMode::NoneAddressing),
    // SEC (set carry flag)
    OpCode::new(0x38, "SEC", 1, 2, AddressingMode::NoneAddressing),
    // CLC (clear carry flag)
    OpCode::new(0x18, "CLC", 1, 2, AddressingMode::NoneAddressing),
    // SED (set decimal flag)
    OpCode::new(0xf8, "SED", 1, 2, AddressingMode::NoneAddressing),
    // CLD (clear decimal flag)
    OpCode::new(0xd8, "CLD", 1, 2, AddressingMode::NoneAddressing),
    // SEI (set interrupt disable flag)
    OpCode::new(0x78, "SEI", 1, 2, AddressingMode::NoneAddressing),
    // CLI (clear interrupt disable flag)
    OpCode::new(0x58, "CLI", 1, 2, AddressingMode::NoneAddressing),
    // CLV (clear overflow flag)
    OpCode::new(0xb8, "CLV", 1, 2, AddressingMode::NoneAddressing),
    // Branch opcodes
    // branch succeeded -> +1, page crossed -> +1
    // BEQ (branch if zero flag is set)
    OpCode::new(0xf0, "BEQ", 2, 2, AddressingMode::NoneAddressing),
    // BNE (branch if zero flag is clear)
    OpCode::new(0xd0, "BNE", 2, 2, AddressingMode::NoneAddressing),
    // BPL (branch if negative flag is clear)
    OpCode::new(0x10, "BPL", 2, 2, AddressingMode::NoneAddressing),
    // BMI (branch if negative flag is set)
    OpCode::new(0x30, "BMI", 2, 2, AddressingMode::NoneAddressing),
    // BVC (branch if overflow flag is clear)
    OpCode::new(0x50, "BVC", 2, 2, AddressingMode::NoneAddressing),
    // BVS (branch if overflow flag is set)
    OpCode::new(0x70, "BVS", 2, 2, AddressingMode::NoneAddressing),
    // BCC (branch if carry flag is clear)
    OpCode::new(0x90, "BCC", 2, 2, AddressingMode::NoneAddressing),
    // BCS (branch if carry flag is set)
    OpCode::new(0xb0, "BCS", 2, 2, AddressingMode::NoneAddressing),
    // JMP (set program counter)
    OpCode::new(0x4c, "JMP", 3, 3, AddressingMode::NoneAddressing),
    OpCode::new(0x6c, "JMP", 3, 5, AddressingMode::NoneAddressing),
    // JSR (jump to subroutine)
    OpCode::new(0x20, "JSR", 3, 6, AddressingMode::Absolute),
    // RTS (return from subroutine)
    OpCode::new(0x60, "RTS", 1, 6, AddressingMode::NoneAddressing),
    // PHA (push register A to stack)
    OpCode::new(0x48, "PHA", 1, 3, AddressingMode::NoneAddressing),
    // PLA (pull register A from stack)
    OpCode::new(0x68, "PLA", 1, 4, AddressingMode::NoneAddressing),
    // PHP (push processor status to stack)
    OpCode::new(0x08, "PHP", 1, 3, AddressingMode::NoneAddressing),
    // PLP (pull processor status from stack)
    OpCode::new(0x28, "PLP", 1, 4, AddressingMode::NoneAddressing),
    // TXS (transfer register X to stack pointer)
    OpCode::new(0x9a, "TXS", 1, 2, AddressingMode::NoneAddressing),
    // TSX (transfer stack pointer to register X)
    OpCode::new(0xba, "TSX", 1, 2, AddressingMode::NoneAddressing),
    // TAX (transfer register A to register X)
    OpCode::new(0xaa, "TAX", 1, 2, AddressingMode::NoneAddressing),
    // TXA (transfer register X to register A)
    OpCode::new(0x8a, "TXA", 1, 2, AddressingMode::NoneAddressing),
    // TAY (transfer register A to register Y)
    OpCode::new(0xa8, "TAY", 1, 2, AddressingMode::NoneAddressing),
    // TYA (transfer register Y to register A)
    OpCode::new(0x98, "TYA", 1, 2, AddressingMode::NoneAddressing),
    // INX (increment register X)
    OpCode::new(0xe8, "INX", 1, 2, AddressingMode::NoneAddressing),
    // DEX (decrement register X)
    OpCode::new(0xca, "DEX", 1, 2, AddressingMode::NoneAddressing),
    // INY (increment register Y)
    OpCode::new(0xc8, "INY", 1, 2, AddressingMode::NoneAddressing),
    // DEY (decrement register Y)
    OpCode::new(0x88, "DEY", 1, 2, AddressingMode::NoneAddressing),
    // INC (increment a memory held value)
    OpCode::new(0xe6, "INC", 2, 5, AddressingMode::ZeroPage),
    OpCode::new(0xf6, "INC", 2, 6, AddressingMode::ZeroPage_X),
    OpCode::new(0xee, "INC", 3, 6, AddressingMode::Absolute),
    OpCode::new(0xfe, "INC", 3, 7, AddressingMode::Absolute_X),
    // DEC (increment a memory held value)
    OpCode::new(0xc6, "DEC", 2, 5, AddressingMode::ZeroPage),
    OpCode::new(0xd6, "DEC", 2, 6, AddressingMode::ZeroPage_X),
    OpCode::new(0xce, "DEC", 3, 6, AddressingMode::Absolute),
    OpCode::new(0xde, "DEC", 3, 7, AddressingMode::Absolute_X),
    // LDA (load to register A)
    OpCode::new(0xa9, "LDA", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xa5, "LDA", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xb5, "LDA", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0xad, "LDA", 3, 4, AddressingMode::Absolute),
    OpCode::new(0xbd, "LDA", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0xb9, "LDA", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0xa1, "LDA", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0xb1, "LDA", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // LDX (load to register X)
    OpCode::new(0xa2, "LDX", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xa6, "LDX", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xb6, "LDX", 2, 4, AddressingMode::ZeroPage_Y),
    OpCode::new(0xae, "LDX", 3, 4, AddressingMode::Absolute),
    OpCode::new(0xbe, "LDX", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    // LDY (load to register Y)
    OpCode::new(0xa0, "LDY", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xa4, "LDY", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xb4, "LDY", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0xac, "LDY", 3, 4, AddressingMode::Absolute),
    OpCode::new(0xbc, "LDY", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    // STA (store register A in memory)
    OpCode::new(0x85, "STA", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x95, "STA", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0x8d, "STA", 3, 4, AddressingMode::Absolute),
    OpCode::new(0x9d, "STA", 3, 5, AddressingMode::Absolute_X),
    OpCode::new(0x99, "STA", 3, 5, AddressingMode::Absolute_Y),
    OpCode::new(0x81, "STA", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0x91, "STA", 2, 6, AddressingMode::Indirect_Y),
    // STX (store register X in memory)
    OpCode::new(0x86, "STX", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x96, "STX", 2, 4, AddressingMode::ZeroPage_Y),
    OpCode::new(0x8e, "STX", 3, 4, AddressingMode::Absolute),
    // STY (store register Y in memory)
    OpCode::new(0x84, "STY", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x94, "STY", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0x8c, "STY", 3, 4, AddressingMode::Absolute),
    // AND (perform a logical AND on register A)
    OpCode::new(0x29, "AND", 2, 2, AddressingMode::Immediate),
    OpCode::new(0x25, "AND", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x35, "AND", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0x2d, "AND", 3, 4, AddressingMode::Absolute),
    OpCode::new(0x3d, "AND", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0x39, "AND", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0x21, "AND", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0x31, "AND", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // ORA (perform a logical OR on register A)
    OpCode::new(0x09, "ORA", 2, 2, AddressingMode::Immediate),
    OpCode::new(0x05, "ORA", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x15, "ORA", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0x0d, "ORA", 3, 4, AddressingMode::Absolute),
    OpCode::new(0x1d, "ORA", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0x19, "ORA", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0x01, "ORA", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0x11, "ORA", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // EOR (perform a logical XOR on register A)
    OpCode::new(0x49, "EOR", 2, 2, AddressingMode::Immediate),
    OpCode::new(0x45, "EOR", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x55, "EOR", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0x4d, "EOR", 3, 4, AddressingMode::Absolute),
    OpCode::new(0x5d, "EOR", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0x59, "EOR", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0x41, "EOR", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0x51, "EOR", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // CMP (compare register A with memory)
    OpCode::new(0xc9, "CMP", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xc5, "CMP", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xd5, "CMP", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0xcd, "CMP", 3, 4, AddressingMode::Absolute),
    OpCode::new(0xdd, "CMP", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0xd9, "CMP", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0xc1, "CMP", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0xd1, "CMP", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // CPX (compare register X with memory)
    OpCode::new(0xe0, "CPX", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xe4, "CPX", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xec, "CPX", 3, 4, AddressingMode::Absolute),
    // CPY (compare register Y with memory)
    OpCode::new(0xc0, "CPY", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xc4, "CPY", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xcc, "CPY", 3, 4, AddressingMode::Absolute),
    // ADC (add to register A with carry-in)
    OpCode::new(0x69, "ADC", 2, 2, AddressingMode::Immediate),
    OpCode::new(0x65, "ADC", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x75, "ADC", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0x6d, "ADC", 3, 4, AddressingMode::Absolute),
    OpCode::new(0x7d, "ADC", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0x79, "ADC", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0x61, "ADC", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0x71, "ADC", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // SBC (substract from register A with borrow-in)
    OpCode::new(0xe9, "SBC", 2, 2, AddressingMode::Immediate),
    OpCode::new(0xe5, "SBC", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0xf5, "SBC", 2, 4, AddressingMode::ZeroPage_X),
    OpCode::new(0xed, "SBC", 3, 4, AddressingMode::Absolute),
    OpCode::new(0xfd, "SBC", 3, 4, AddressingMode::Absolute_X), // page crossed -> 5
    OpCode::new(0xf9, "SBC", 3, 4, AddressingMode::Absolute_Y), // page crossed -> 5
    OpCode::new(0xe1, "SBC", 2, 6, AddressingMode::Indirect_X),
    OpCode::new(0xf1, "SBC", 2, 5, AddressingMode::Indirect_Y), // page crossed -> 6
    // BIT (test if bits are set in memory)
    OpCode::new(0x24, "BIT", 2, 3, AddressingMode::ZeroPage),
    OpCode::new(0x2c, "BIT", 3, 4, AddressingMode::Absolute),
];

pub static OPCODES_MAP: LazyLock<HashMap<u8, &'static OpCode>> =
    LazyLock::new(|| CPU_OPCODES.iter().map(|op| (op.code, op)).collect());

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_opcode_table_mode_length_consistency() {
        for op in CPU_OPCODES {
            let expected_len = match op.mode {
                AddressingMode::NoneAddressing => op.len,
                AddressingMode::Immediate
                | AddressingMode::ZeroPage
                | AddressingMode::ZeroPage_X
                | AddressingMode::ZeroPage_Y
                | AddressingMode::Indirect_X
                | AddressingMode::Indirect_Y => 2,
                AddressingMode::Absolute
                | AddressingMode::Absolute_X
                | AddressingMode::Absolute_Y => 3,
            };
            assert_eq!(
                op.len, expected_len,
                "opcode 0x{:02x} ({}) declared mode {:?} but len {} (expected {})",
                op.code, op.mnemonic, op.mode, op.len, expected_len
            );
        }
    }
}
