# Nessie

This is my first Rust project: A simple NES Emulator written in Rust.

Created by following this awesome guide: [https://github.com/bugzmanov/nes_ebook](https://github.com/bugzmanov/nes_ebook)

It's not finished yet (missing APU, mappers etc.), but it's able to run simple games like Pac-Man or Alter Ego.

## Setup

1. Install cargo & SDL2
2. Update `main.rs` to load your own rom
3. Use `cargo run --release` for an optimized build

> **Important:** Only tested on macOS 26

## Controls

* **UP, DOWN, LEFT, RIGHT:** Arrow Keys
* **SELECT:** Space
* **START:** Enter
* **A:** A
* **B:** S
* Quit with Escape
