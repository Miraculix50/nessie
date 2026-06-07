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
                AddressingMode::NoneAddressing => 1,
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
