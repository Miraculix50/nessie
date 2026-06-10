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

### CMP

CMP introduces reading the **Carry flag as a result** (in addition to setting Z/N). Since three distinct outcomes (A==M, A>M, A<M) each set a unique combination of C and Z, this needs more than one test to cover the new flag transitions. All addressing modes are still shared with LDA, so we don't repeat those tests.

| Test | Purpose |
|---|---|
| `test_cmp_equal_sets_z_and_c` | `LDA #$05; CMP #$05` → A unchanged, Z=1, C=1, N=0. Verifies the equal case and that A is preserved (CMP must not write back to A). |
| `test_cmp_less_than_with_carry_set_clears_c` | `SEC; LDA #$00; CMP #$01` → A unchanged, C=0, Z=0, N=1. The `SEC` before CMP is critical: it seeds C=1 so a buggy CMP that forgets to clear C would fail the final `C==0` assertion (catch the "CMP doesn't write C" bug). Verifies the A<M case. |

### ADC

The first "big" instruction: A = A + M + C, affecting **all four** of C, Z, V, and N. The V (signed overflow) flag is the trickiest part of the 6502 to emulate, so it needs explicit coverage. Carry-in is also new — it's the first instruction that *reads* the C flag, so a buggy ADC that ignores C-in will pass any test where C happens to be 0.

| Test | Purpose |
|---|---|
| `test_adc_basic_add_no_carry_in` | `LDA #$05; ADC #$03` → A=8, C=0, Z=0, V=0, N=0. The "happy path" baseline. C starts at 0 after reset, so this test only checks that plain 8-bit addition works and the four flags default to clear. |
| `test_adc_adds_carry_in` | `SEC; LDA #$05; ADC #$03` → A=9. Catches the "ADC ignored the carry-in" bug class: a buggy implementation that doesn't read `status & 0x01` would produce 8, not 9. |
| `test_adc_carry_out_sets_c_and_zero_result_sets_z` | `LDA #$FF; ADC #$01` → A=0, C=1, Z=1. Exercises unsigned overflow (C=1) and zero result (Z=1) in the same operation, plus confirms V=0 (no signed overflow for this input pair). |
| `test_adc_signed_overflow_sets_v` | `LDA #$50; ADC #$50` → A=0xA0, V=1, N=1, C=0, Z=0. The classic 6502 V-flag test: (+80) + (+80) = 0xA0, which is −96 in signed, so signed overflow occurred and V must be set. Result also has bit 7 set (N=1). This is the test that catches an incorrect V formula (e.g. one that tests bit 6 carry without XOR-ing bit 7 carry). |

### SBC

Subtraction counterpart to ADC. Semantics: A = A − M − !C. Note that C is **inverted** — C=1 means "no borrow". This is the first instruction that affects all four of C, Z, V, N like ADC does, but the borrow-in is the conceptually tricky part.

The V formula for subtraction is different from ADC: it uses `((A ^ M) & (A ^ R) & 0x80) != 0` rather than `((A ^ R) & (M ^ R) & 0x80) != 0`. Both detect signed overflow but from different angles (ADC's checks the sign of both inputs changed; SBC's checks the sign of A and the result differ while A and M had different signs).

| Test | Purpose |
|---|---|
| `test_sbc_basic_subtract_no_borrow_in` | `SEC; LDA #$05; SBC #$03` → A=2, C=1, Z=0, V=0, N=0. The "happy path": C=1 pre-loaded (no borrow coming in), plain 8-bit subtraction, no overflow anywhere. |
| `test_sbc_subtracts_borrow_in` | `CLC; LDA #$05; SBC #$03` → A=1, C=1. Catches the "SBC ignored the carry-in" bug class: a buggy SBC that doesn't subtract `!C` would produce 2, not 1. |
| `test_sbc_underflow_clears_c_and_sets_n` | `SEC; LDA #$00; SBC #$01` → A=0xFF, C=0, Z=0, V=0, N=1. Exercises borrow-out (C=0) and a negative result (N=1). V=0 because 0−1 = −1 is a valid signed 8-bit value. |
| `test_sbc_signed_overflow_sets_v` | `SEC; LDA #$80; SBC #$01` → A=0x7F, V=1, C=1, Z=0, N=0. In signed: (−128) − (+1) = −129, which doesn't fit in 8-bit signed, so V must be set. Catches an incorrect V formula for subtraction. |

### ORA

Bitwise OR, structural mirror of AND. All 8 addressing modes are reused from LDA/AND, and flag handling is the same `update_zero_and_negative_flags` helper.

| Test | Purpose |
|---|---|
| `test_ora_immediate` | `LDA #$AA; ORA #$55` → A=0xFF, N=1, Z=0. Pairs complementing inputs so every bit is exercised (10101010 | 01010101 = 11111111). A bug that ORed with 0 or wrote back to the wrong register would produce A != 0xFF. |

### EOR

Bitwise XOR, structural mirror of AND/ORA. Completes the logical triplet.

| Test | Purpose |
|---|---|
| `test_eor_immediate` | `LDA #$AA; EOR #$FE` → A=0x54. The result is distinct from ORA (0xFE) and AND (0xAA) for the same inputs, proving EOR is wired correctly and not accidentally aliased to another logical op. |

### CLD & SED (Decimal flag)

Decimal mode (D, bit 3) is vestigial on the NES's 2A03 CPU — the hardware ignores it during ADC/SBC — but `CLD`/`SED` instructions still set and clear the flag at the status-register level. Software relies on reading/writing D for state preservation.

| Test | Purpose |
|---|---|
| `test_sed_sets_decimal_flag` | D starts at 0 after reset; `SED` must set bit 3. |
| `test_cld_clears_decimal_flag` | `SED; CLD` exercises both transitions; final assertion is D=0. |

### CLI & SEI (Interrupt disable flag)

The I flag (bit 2, `0b0000_0100`) controls whether maskable IRQs are acknowledged. Currently unused by the emulator (no IRQ hardware exists yet), but `CLI`/`SEI` appear in almost all real NES code and must be emulated at the status-register level.

| Test | Purpose |
|---|---|
| `test_sei_sets_interrupt_flag` | I starts at 0 after reset; `SEI` must set bit 2. |
| `test_cli_clears_interrupt_flag` | `SEI; CLI` exercises both transitions; final assertion is I=0. |

### CLV (Overflow flag)

CLV is unique among the flag setters — there's no `SEV` to pair with it. The V flag (bit 6) is normally set by arithmetic (`ADC`/`SBC` overflow) or by `BIT`. To test CLV we borrow ADC's V-output to seed `V=1` first.

| Test | Purpose |
|---|---|
| `test_clv_clears_overflow_flag` | `LDA #$50; ADC #$50` sets V=1 via signed overflow, then `CLV` clears it. Also asserts that N and Z (set by the ADC) survive CLV intact, proving CLV is not accidentally clearing the entire status byte. |

### CPX & CPY (compare X / compare Y)

Like CMP but for the X and Y registers. Each uses 3 addressing modes (Immediate, ZP, Absolute) — all already covered by LDA/CMP tests. Flag logic is inherited from CMP.

| Test | Purpose |
|---|---|
| `test_cpx_equal_sets_z_and_c` | `LDX #$42; CPX #$42` → equal: Z=1, C=1, N=0. X must not be modified. |
| `test_cpy_equal_sets_z_and_c` | `LDY #$42; CPY #$42` → same as above. Y must not be modified. |

### INC & DEC (increment/decrement memory)

First instructions that read, modify, **and write back** to memory — a new pattern not exercised by any existing instruction. All existing opcodes either read from memory (LDA, AND, ADC, ...) or write to memory (STA, STX, STY, ...), but none do both to the same address.

Both use 4 addressing modes (ZP, ZP,X, Absolute, Absolute,X) — all shared with existing instructions, so the resolver is already covered.

| Test | Purpose |
|---|---|
| `test_inc_increments_memory` | Pre-writes `0x01` at `$10`, runs `INC $10` (`$E6`). Asserts the byte was read, incremented, and the result (`0x02`) was written back to the same address. |
| `test_dec_decrements_memory` | Pre-writes `0x02` at `$10`, runs `DEC $10` (`$C6`). Same read-modify-write assertion but for subtraction. |

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
