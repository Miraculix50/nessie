# CPU Test Plan

This document describes the testing strategy for the CPU module (`src/cpu.rs`).
It is intended for future contributors (human or AI) to understand **what is
tested, what is intentionally not tested, and why**, so that tests can be
extended consistently as new instructions and features are added.

## Guiding principles

1. **Cover every code path at least once**, but avoid redundant tests for code
   that is already exercised through a shared helper.
2. **Prefer testing through shared helpers via the simplest caller**. For
   example, `get_operand_address` is tested via LDA because the result is
   directly observable in `register_a`.
3. **Test edge cases that historically cause emulator bugs** — wrap-arounds,
   little-endian byte order, flag transitions on overflow.
4. **Integration tests verify composition**, not individual ops.

## Included tests

### Memory primitives (`Mem` trait)

| Test | Purpose |
|------|---------|
| `test_mem_read_write` | Round-trip a single byte through `mem_read` / `mem_write`. Foundation for every other test. |
| `test_mem_read_write_u16_little_endian` | Verifies 16-bit values are stored low byte first. The 6502 is little-endian and many subtle bugs hide here. |

### CPU lifecycle

| Test | Purpose |
|------|---------|
| `test_reset_clears_registers_and_loads_pc` | `reset()` must zero A/X/status and load PC from the reset vector at `0xFFFC`. |
| `test_load_places_program_at_0x8000_and_sets_reset_vector` | `load()` must copy the program to `0x8000` and write `0x8000` into the reset vector so `load_and_run` starts correctly. |

### LDA — one test per addressing mode

LDA is the canonical way to exercise `get_operand_address` because the
fetched value lands directly in `register_a`, which is trivial to assert on.
**Adding new instructions does not require duplicating these tests** — if the
addressing mode works for LDA, it works for any instruction that calls
`get_operand_address`.

| Test | Mode | Notes |
|------|------|-------|
| `test_lda_immediate` | Immediate | Baseline. |
| `test_lda_zero_page` | ZeroPage | |
| `test_lda_zero_page_x_with_wrap` | ZeroPage_X | Also covers `wrapping_add` when `pos + X` overflows a `u8`. |
| `test_lda_absolute` | Absolute | |
| `test_lda_absolute_x` | Absolute_X | |
| `test_lda_absolute_y` | Absolute_Y | |
| `test_lda_indirect_x_with_wrap` | Indirect_X | Trickiest mode; covers zero-page pointer wrap. |
| `test_lda_indirect_y` | Indirect_Y | |

### Zero & Negative flags

`update_zero_and_negative_flags` is shared by LDA, TAX, and INX. We test it
thoroughly **once** via LDA. Other instructions inherit correctness.

| Test | Purpose |
|------|---------|
| `test_lda_sets_zero_flag_when_zero` | LDA `#0` sets Z, leaves N clear. |
| `test_lda_sets_negative_flag_when_high_bit_set` | LDA `#0x80` sets N, leaves Z clear. |

Flag *clearing* behavior is covered implicitly by the integration test, which
chains multiple flag-modifying operations.

### STA

| Test | Purpose |
|------|---------|
| `test_sta_writes_a_to_memory` | Confirms STA writes `register_a` to the address resolved by `get_operand_address`. Only one addressing mode is exercised because the address resolution path is already covered by LDA tests. |

### LDX

| Test | Purpose |
|------|---------|
| `test_ldx_immediate` | Functional test that `$A2` is wired through the run loop and lands the value in `register_x`. |
| `test_ldx_zero_page_y_with_wrap` | Covers the previously-untested `ZeroPage_Y` addressing mode, including `pos + Y` wrap-around (`$FF + $02 → $01`). Uses TAY to set Y from inside the program (since LDY is not yet implemented). |

### LDY

| Test | Purpose |
|------|---------|
| `test_ldy_immediate` | Functional test that `$A0` is wired through the run loop and lands the value in `register_y`. Every LDY addressing mode (Immediate, ZP, ZP,X, Absolute, Absolute,X) is already exercised by the LDA tests through the shared `get_operand_address` resolver, so only the wiring is verified here. |

### STX

| Test | Purpose |
|------|---------|
| `test_stx_writes_x_to_memory` | Verifies `$86` is wired through the run loop and stores `register_x` to the resolved address. The test first loads a non-zero value into X via `TAX` so a passing assertion cannot be coincidence (X is 0 after reset). Other STX modes (ZeroPage,Y, Absolute, Absolute,Y) share the `get_operand_address` resolver already tested via LDX/LDA/STA tests. |

### STY

| Test | Purpose |
|------|---------|
| `test_sty_writes_y_to_memory` | Verifies `$84` is wired through the run loop and stores `register_y` to the resolved address. Same anti-coincidence pattern as `test_stx_writes_x_to_memory`: `TAY` is used to set a non-zero Y before the store, since Y is 0 after reset. |

### CLC & SEC (Carry flag)

These are the first instructions that touch a flag other than Z/N (the only flags currently covered by `update_zero_and_negative_flags`). The Carry flag is bit 0 (`0b0000_0001`) of the status register, and reading/writing it correctly is a prerequisite for ADC, SBC, shifts, and conditional branches.

| Test | Purpose |
|------|---------|
| `test_sec_sets_carry_flag` | C starts at 0 after reset; `SEC` must set bit 0. Verifies the `set` transition. |
| `test_clc_clears_carry_flag` | `SEC; CLC` exercises both the set and the clear transition; final assertion is C=0. Verifies the `clear` path actually works (a buggy CLC that OR'd in 1 by mistake would fail this). |

### AND

This is the first instruction that does real data manipulation (A = A & M). All 8 addressing modes are reused from LDA, and Z/N flag handling comes from the existing `update_zero_and_negative_flags` helper, so per the rules we need only one functional test.

| Test | Purpose |
|------|---------|
| `test_and_immediate` | `LDA #$AA; AND #$55` → result `0x00`, Z flag set. Picked a non-zero-zero input pair that produces a *zero* result so the test simultaneously exercises the wiring, the bitwise AND, and the Z-flag transition (a buggy AND that forgets to call `update_zero_and_negative_flags` would fail). All other addressing modes are already covered by LDA tests through the shared `get_operand_address` resolver. |

### Register transfers & increments

| Test | Purpose |
|------|---------|
| `test_tax_transfers_a_to_x` | Basic register transfer. |
| `test_tay_transfers_a_to_y` | Basic A → Y transfer. Flag behavior is inherited from the shared `update_zero_and_negative_flags` helper (tested via LDA), so no separate flag tests are needed. |
| `test_txa_transfers_x_to_a` | Basic X → A transfer. The program overwrites A between `TAX` and `TXA` (via `LDA #$00`) so the final A value can only come from X, proving the transfer actually happened. |
| `test_tya_transfers_y_to_a` | Basic Y → A transfer. Same anti-coincidence pattern as `test_txa_transfers_x_to_a`: A is clobbered between `TAY` and `TYA` so a passing assertion can only come from a real Y → A transfer. |
| `test_inx_increments_x` | Basic increment. |
| `test_inx_overflow_wraps_to_zero` | `0xFF + 1` must wrap to `0x00` and set Z. Catches a common emulator bug. |
| `test_iny_increments_y` | Basic Y increment via `LDA #$05; TAY; INY`. Wrap-around behavior is already covered by `test_inx_overflow_wraps_to_zero` through the shared `update_zero_and_negative_flags` helper, so only the wiring is verified here. |
| `test_dex_underflow_wraps_to_0xff` | `0x00 - 1` must wrap to `0xFF` and set N. This single test covers the new `wrapping_sub` behavior, the opcode wiring, and the N-flag transition. Basic non-underflow decrement is not separately tested because it shares the same code path. |
| `test_dey_decrements_y` | Basic Y decrement via `LDA #$05; TAY; DEY`. Underflow / `wrapping_sub` behavior already covered by `test_dex_underflow_wraps_to_0xff`, so only the wiring is verified here. |

### NOP

| Test | Purpose |
|------|---------|
| `test_nop_does_nothing` | Runs `LDA #$42; NOP; BRK` and asserts `register_a == 0x42`. Confirms (a) `$EA` is wired into the run loop without panicking, (b) NOP does not clobber the accumulator. Status preservation is implicit: a buggy NOP that modified flags would also typically modify a register. The opcode-table-integrity test independently verifies that `len: 1` is paired with `NoneAddressing`. |

### Integration

| Test | Purpose |
|------|---------|
| `test_lda_tax_inx_program` | Runs `LDA #0xc0; TAX; INX; BRK`. Verifies multiple instructions execute in sequence, the program counter advances correctly, and flag *transitions* across operations behave as expected. |

### Opcode table integrity (`src/opcodes.rs`)

| Test | Purpose |
|------|---------|
| `test_opcode_table_mode_length_consistency` | Iterates `CPU_OPCODES` and asserts that each entry's `len` matches the byte-count required by its `AddressingMode` (1 for `NoneAddressing`, 2 for immediate/zero-page/indirect modes, 3 for absolute modes). This catches copy-paste errors in the opcode table that the addressing-mode resolver tests cannot detect, because the resolver is tested via LDA entries only — bugs in any other instruction's table row (e.g. a wrong `mode` paired with a correct `len`) would otherwise go unnoticed until that opcode is exercised at runtime. |

## Intentionally excluded tests

These are **not** missing by accident. Do not add them without good reason.

| Excluded | Why |
|----------|-----|
| STA tests for every addressing mode | `get_operand_address` is already exhaustively tested via LDA. Duplicating it for STA tests nothing new. The *opcode table* itself (which mode each STA byte declares) is covered by `test_opcode_table_mode_length_consistency` in `opcodes.rs`. |
| Per-instruction flag tests for TAX / INX (beyond the INX overflow case) | They share `update_zero_and_negative_flags` with LDA, which is tested directly. The INX overflow test is kept because the wrap-around is a separate behavior. |
| A `#[should_panic]` test for `AddressingMode::NoneAddressing` | The opcode table never pairs `NoneAddressing` with an instruction that calls `get_operand_address`, so the panic is unreachable defensive code. |
| A dedicated BRK test | BRK is implicitly exercised by every test — they all rely on it to halt the `run` loop. |
| Cycle-count assertions | Cycles are declared in `OPCODES_MAP` but the CPU does not currently consume or emit them. Add tests when cycle accounting is implemented. |
| A test verifying every opcode in `CPU_OPCODES` has a dispatch arm in `run()` | The bug class (table entry exists, match arm missing) is caught for free by per-instruction functional tests — the first time the opcode is executed, the `_ => unimplemented!(...)` arm in `run()` panics with `opcode 0x{:02x} ({}) has no dispatch arm in run()`, naming the missing handler via the table's `mnemonic` field. A standalone meta-test would either need brittle `catch_unwind` plumbing or duplicate the match's structure. Once the dispatch grows unwieldy, the right fix is a structural refactor (e.g. function-pointer dispatch table on `OpCode`), not a test. |

## Guidance for adding new tests

When adding a new instruction:

1. **New instruction reusing existing addressing modes and flag helper:**
   one functional test is sufficient. Do not re-test addressing modes or flags.
   The opcode table integrity test will automatically validate the new entries'
   `mode`/`len` consistency.
2. **New instruction introducing a new addressing mode:** add one LDA-style
   test for the new mode (or for the new instruction if LDA does not support it).
3. **New instruction introducing new flags (C, V, I, D, B):** add focused
   tests for set/clear transitions of each new flag, analogous to the existing
   Z/N tests.
4. **New behavior that composes multiple instructions** (e.g. interrupts,
   stack ops): add a new integration-style test alongside `test_lda_tax_inx_program`.
