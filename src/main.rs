pub mod bus;
pub mod cartridge;
pub mod cpu;
pub mod joypad;
pub mod opcodes;
pub mod ppu;
pub mod render;
pub mod tile_viewer;
pub mod trace;

use sdl2::EventPump;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

use crate::bus::Bus;
use crate::cartridge::Rom;
use crate::cpu::CPU;
use crate::joypad::JoypadButton;
use crate::render::frame::Frame;

const SCALE: f64 = 2.0;
const WIDTH: u32 = 256;
const HEIGHT: u32 = 240;

fn handle_input(cpu: &mut CPU, event_pump: &mut EventPump) {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => std::process::exit(0),
            Event::KeyDown { keycode, .. } => match keycode {
                Some(Keycode::Down) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::DOWN, true),
                Some(Keycode::Up) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::UP, true),
                Some(Keycode::Right) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::RIGHT, true),
                Some(Keycode::Left) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::LEFT, true),
                Some(Keycode::Space) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::SELECT, true),
                Some(Keycode::Return) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::START, true),
                Some(Keycode::A) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::BUTTON_A, true),
                Some(Keycode::S) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::BUTTON_B, true),
                _ => {}
            },
            Event::KeyUp { keycode, .. } => match keycode {
                Some(Keycode::Down) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::DOWN, false),
                Some(Keycode::Up) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::UP, false),
                Some(Keycode::Right) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::RIGHT, false),
                Some(Keycode::Left) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::LEFT, false),
                Some(Keycode::Space) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::SELECT, false),
                Some(Keycode::Return) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::START, false),
                Some(Keycode::A) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::BUTTON_A, false),
                Some(Keycode::S) => cpu
                    .bus
                    .joypad1
                    .set_button_pressed_status(JoypadButton::BUTTON_B, false),
                _ => {}
            },
            _ => {}
        }
    }
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window(
            "Nessie",
            (WIDTH as f64 * SCALE) as u32,
            (HEIGHT as f64 * SCALE) as u32,
        )
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().present_vsync().build().unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();
    canvas.set_scale(SCALE as f32, SCALE as f32).unwrap();

    let creator = canvas.texture_creator();
    let mut texture = creator
        .create_texture_target(PixelFormatEnum::RGB24, WIDTH, HEIGHT)
        .unwrap();

    let bytes = std::fs::read("roms/nestest.nes").unwrap();
    let rom = Rom::new(&bytes).unwrap();
    let bus = Bus::new(rom);
    let mut cpu = CPU::new(bus);
    cpu.reset();
    let mut frame = Frame::new();

    cpu.run_with_callback(|cpu| {
        if cpu.bus.frame_ready {
            render::render(&cpu.bus.ppu, &mut frame);
            cpu.bus.frame_ready = false;

            texture
                .update(None, &frame.data, WIDTH as usize * 3)
                .unwrap();
            canvas.copy(&texture, None, None).unwrap();
            canvas.present();
        }

        handle_input(cpu, &mut event_pump);
    });
}
