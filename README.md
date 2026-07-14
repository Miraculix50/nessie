# Nessie

This is my first Rust project: A simple NES Emulator written in Rust.

Created by following this awesome guide: [https://github.com/bugzmanov/nes_ebook](https://github.com/bugzmanov/nes_ebook)

It's not finished yet (missing APU, mappers etc.), but it's able to run simple games like Pac-Man or Alter Ego.

## Setup

1. Install cargo & SDL2 (statically compiled by default, requires a C compiler)
2. Get the file path to your rom
3. Use `ROM_PATH=path_to_your_rom cargo run --release` for an optimized build

> **Important:** Only tested on macOS 26

## Controls

* **UP, DOWN, LEFT, RIGHT:** Arrow Keys
* **SELECT:** Space
* **START:** Enter
* **A:** A
* **B:** S
* Quit with Escape
