extern crate sdl2;

use rand;
use sdl2::audio::{AudioCallback, AudioSpecDesired};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::WindowCanvas;
use sdl2::video::Window;
use std::ops::Add;
use std::time::Duration;

const SCALE: u32 = 20;
const GRID_X_SIZE: u32 = 64;
const GRID_Y_SIZE: u32 = 32;
const DOT_SIZE_IN_PXS: u32 = 1 * SCALE;

const KEYMAP: [(Keycode, u32); 16] = [
    (Keycode::Num1, 0x1), // 1
    (Keycode::Num2, 0x2), // 2
    (Keycode::Num3, 0x3), // 3
    (Keycode::Num4, 0xc), // 4
    (Keycode::Q, 0x4),    // Q
    (Keycode::W, 0x5),    // W
    (Keycode::E, 0x6),    // E
    (Keycode::R, 0xd),    // R
    (Keycode::A, 0x7),    // A
    (Keycode::S, 0x8),    // S
    (Keycode::D, 0x9),    // D
    (Keycode::F, 0xe),    // F
    (Keycode::Z, 0xa),    // Z
    (Keycode::X, 0x0),    // X
    (Keycode::C, 0xb),    // C
    (Keycode::V, 0xf),    // V
];

pub enum EmulatorState {
    Playing,
    Paused,
}

#[derive(Copy, Clone, PartialEq)]
pub struct Point(pub i32, pub i32);

impl Add<Point> for Point {
    type Output = Point;

    fn add(self, rhs: Point) -> Self::Output {
        Point(self.0 + rhs.0, self.1 + rhs.1)
    }
}

pub struct EmulatorContext {
    pub keyboard: Keyboard,
    pub display: Vec<Point>,
    pub state: EmulatorState,
    pub memory: [u8; 4096],
    pub registers: [u8; 16],
    pub i: u16,
    pub delay_timer: u8,
    pub sound_timer: u8,
    pub pc: u16,
    pub stack: Vec<u16>,
}

impl EmulatorContext {
    pub fn new() -> EmulatorContext {
        EmulatorContext {
            keyboard: Keyboard::new(),
            display: vec![],
            state: EmulatorState::Playing,
            memory: [0; 4096],
            registers: [0; 16],
            i: 0,
            delay_timer: 0,
            sound_timer: 0,
            pc: 0x200,
            stack: Vec::new(),
        }
    }
    pub fn load_sprites_into_memory(&mut self) {
        let sprites = [
            0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
            0x20, 0x60, 0x20, 0x20, 0x70, // 1
            0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
            0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
            0x90, 0x90, 0xF0, 0x10, 0x10, // 4
            0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
            0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
            0xF0, 0x10, 0x20, 0x40, 0x40, // 7
            0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
            0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
            0xF0, 0x90, 0xF0, 0x90, 0x90, // A
            0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
            0xF0, 0x80, 0x80, 0x80, 0xF0, // C
            0xE0, 0x90, 0x90, 0x90, 0xE0, // D
            0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
            0xF0, 0x80, 0xF0, 0x80, 0x80, // F
        ];
        for (i, &sprite) in sprites.iter().enumerate() {
            self.memory[i] = sprite;
        }
    }
    pub fn load_program_into_memory(&mut self) {
        // let program = include_bytes!("../roms/TETRIS");
        let program = include_bytes!("../roms/SUBMARINE");
        // let program = include_bytes!("../roms/PONG");
        // let program = include_bytes!("../roms/ANIMAL_RACE");
        // let program = include_bytes!("../roms/chip8-test-suite/bin/5-quirks.ch8");
        for (i, &byte) in program.iter().enumerate() {
            self.memory[i + 0x200] = byte;
        }
    }

    pub fn cycle(&mut self) {
        if let EmulatorState::Paused = self.state {
            return;
        }
        for _ in 0..10 {
            self.execute_opcode();
        }
        self.update_timers();
    }
    pub fn execute_opcode(&mut self) {
        if let EmulatorState::Paused = self.state {
            return;
        }
        let opcode =
            (self.memory[self.pc as usize] as u16) << 8 | self.memory[self.pc as usize + 1] as u16;
        self.pc += 2;
        println!("{:04X}", opcode);
        match opcode & 0xF000 {
            0x0000 => match opcode & 0x00FF {
                0xE0 => {
                    // Clear the display
                    self.display.clear();
                }
                0xEE => {
                    // Return from a subroutine
                    self.pc = self.stack.pop().unwrap();
                }
                _ => {}
            },
            0x1000 => {
                // Jump to address NNN
                self.pc = opcode & 0x0FFF;
            }
            0x2000 => {
                // Call subroutine at NNN
                self.stack.push(self.pc);
                self.pc = opcode & 0x0FFF;
            }
            0x3000 => {
                // Skip next instruction if Vx == NN
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let nn = (opcode & 0x00FF) as u8;
                if self.registers[x] == nn {
                    self.pc += 2;
                }
            }
            0x4000 => {
                // Skip next instruction if Vx != NN
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let nn = (opcode & 0x00FF) as u8;
                if self.registers[x] != nn {
                    self.pc += 2;
                }
            }
            0x5000 => {
                // Skip next instruction if Vx == Vy
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let y = ((opcode & 0x00F0) >> 4) as usize;
                if self.registers[x] == self.registers[y] {
                    self.pc += 2;
                }
            }
            0x6000 => {
                // Set Vx = NN
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let nn = (opcode & 0x00FF) as u8;
                self.registers[x] = nn;
            }
            0x7000 => {
                // Set Vx = Vx + NN
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let nn = (opcode & 0x00FF) as u8;
                self.registers[x] = self.registers[x].wrapping_add(nn);
            }
            0x8000 => match opcode & 0x000F {
                0x0000 => {
                    // Set Vx = Vy
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.registers[x] = self.registers[y];
                }
                0x0001 => {
                    // Set Vx = Vx OR Vy
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.registers[x] |= self.registers[y];
                    self.registers[0xF] = 0;
                }
                0x0002 => {
                    // Set Vx = Vx AND Vy
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.registers[x] &= self.registers[y];
                    self.registers[0xF] = 0;
                }
                0x0003 => {
                    // Set Vx = Vx XOR Vy
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.registers[x] ^= self.registers[y];
                    self.registers[0xF] = 0;
                }
                0x0004 => {
                    // Set Vx = Vx + Vy, set VF = carry
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    let sum = self.registers[x] as u16 + self.registers[y] as u16;
                    self.registers[x] = sum as u8;
                    self.registers[0xF] = if sum > 0xFF { 1 } else { 0 };
                }
                0x0005 => {
                    // Set Vx = Vx - Vy, set VF = NOT borrow
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.registers[0xF] = if self.registers[x] > self.registers[y] {
                        1
                    } else {
                        0
                    };
                    self.registers[x] = self.registers[x].wrapping_sub(self.registers[y]);
                }
                0x0006 => {
                    // Set Vx = Vx SHR 1
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.registers[0xF] = self.registers[x] & 0x1;
                    self.registers[x] >>= 1;
                }
                0x0007 => {
                    // Set Vx = Vy - Vx, set VF = NOT borrow
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let y = ((opcode & 0x00F0) >> 4) as usize;
                    self.registers[0xF] = if self.registers[y] > self.registers[x] {
                        1
                    } else {
                        0
                    };
                    self.registers[x] = self.registers[y].wrapping_sub(self.registers[x]);
                }
                0x000E => {
                    // Set Vx = Vx SHL 1
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.registers[0xF] = self.registers[x] >> 7;
                    self.registers[x] <<= 1;
                }
                _ => {}
            },
            0x9000 => {
                // Skip next instruction if Vx != Vy
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let y = ((opcode & 0x00F0) >> 4) as usize;
                if self.registers[x] != self.registers[y] {
                    self.pc += 2;
                }
            }
            0xA000 => {
                // Set I = NNN
                self.i = opcode & 0x0FFF;
            }
            0xB000 => {
                // Jump to location NNN + V0
                self.pc = (opcode & 0x0FFF) + self.registers[0] as u16;
            }
            0xC000 => {
                // Set Vx = random byte AND NN
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let nn = (opcode & 0x00FF) as u8;
                self.registers[x] = rand::random::<u8>() & nn;
            }
            0xD000 => {
                // Display n-byte sprite starting at memory location I at (Vx, Vy), set VF = collision
                let x = ((opcode & 0x0F00) >> 8) as usize;
                let y = ((opcode & 0x00F0) >> 4) as usize;
                let n = (opcode & 0x000F) as usize;
                let vx = self.registers[x] as usize;
                let vy = self.registers[y] as usize;
                self.registers[0xF] = 0;
                for yline in 0..n {
                    let pixel = self.memory[(self.i + yline as u16) as usize];
                    for xline in 0..8 {
                        if (pixel & (0x80 >> xline)) != 0 {
                            if self.display.contains(&Point(
                                vx as i32 + xline as i32,
                                vy as i32 + yline as i32,
                            )) {
                                self.registers[0xF] = 1;
                                self.display.retain(|&p| {
                                    p != Point(vx as i32 + xline as i32, vy as i32 + yline as i32)
                                });
                            } else {
                                self.display.push(Point(
                                    vx as i32 + xline as i32,
                                    vy as i32 + yline as i32,
                                ));
                            }
                        }
                    }
                }
            }
            0xE000 => match opcode & 0x00FF {
                0x009E => {
                    // Skip next instruction if key with the value of Vx is pressed
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    if self.keyboard.is_key_pressed(self.registers[x] as u32) {
                        self.pc += 2;
                    }
                }
                0x00A1 => {
                    // Skip next instruction if key with the value of Vx is not pressed
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    if !self.keyboard.is_key_pressed(self.registers[x] as u32) {
                        self.pc += 2;
                    }
                }
                _ => {}
            },
            0xF000 => match opcode & 0x00FF {
                0x0007 => {
                    // Set Vx = delay timer value
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.registers[x] = self.delay_timer;
                }
                0x000A => {
                    // Wait for a key press, store the value of the key in Vx
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    let mut key_pressed = false;
                    for key in &self.keyboard.keys_pressed {
                        self.registers[x] = *key as u8;
                        key_pressed = true;
                        break;
                    }
                    if !key_pressed {
                        self.pc -= 2;
                    }
                }
                0x0015 => {
                    // Set delay timer = Vx
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.delay_timer = self.registers[x];
                }
                0x0018 => {
                    // Set sound timer = Vx
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.sound_timer = self.registers[x];
                }
                0x001E => {
                    // Set I = I + Vx
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.i += self.registers[x] as u16;
                }
                0x0029 => {
                    // Set I = location of sprite for digit Vx
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.i = self.registers[x] as u16 * 5;
                }
                0x0033 => {
                    // Store BCD representation of Vx in memory locations I, I+1, and I+2
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    self.memory[self.i as usize] = self.registers[x] / 100;
                    self.memory[self.i as usize + 1] = (self.registers[x] % 100) / 10;
                    self.memory[self.i as usize + 2] = self.registers[x] % 10;
                }
                0x0055 => {
                    // Store registers V0 through Vx in memory starting at location I
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    for i in 0..x + 1 {
                        self.memory[self.i as usize + i] = self.registers[i];
                    }
                }
                0x0065 => {
                    // Read registers V0 through Vx from memory starting at location I
                    let x = ((opcode & 0x0F00) >> 8) as usize;
                    for i in 0..x + 1 {
                        self.registers[i] = self.memory[self.i as usize + i];
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
    pub fn update_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }
    pub fn toggle_pause(&mut self) {
        self.state = match self.state {
            EmulatorState::Playing => EmulatorState::Paused,
            EmulatorState::Paused => EmulatorState::Playing,
        }
    }
}

pub struct Speaker {
    phase_inc: f32,
    phase: f32,
    volume: f32,
}

impl AudioCallback for Speaker {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

pub struct Keyboard {
    keys_pressed: Vec<u32>,
}

impl Keyboard {
    pub fn new() -> Keyboard {
        Keyboard {
            keys_pressed: Vec::new(),
        }
    }
    pub fn key_down(&mut self, key: Keycode) {
        self.keys_pressed
            .push(KEYMAP.iter().find(|&&x| x.0 == key).unwrap().1);
    }
    pub fn key_up(&mut self, key: Keycode) {
        self.keys_pressed
            .retain(|&x| x != KEYMAP.iter().find(|&&x| x.0 == key).unwrap().1);
    }
    pub fn is_key_pressed(&self, key: u32) -> bool {
        self.keys_pressed.contains(&key)
    }
}

pub struct Renderer {
    canvas: WindowCanvas,
}

impl Renderer {
    pub fn new(window: Window) -> Result<Renderer, String> {
        let canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
        Ok(Renderer { canvas })
    }
    fn draw_dot(&mut self, point: &Point) -> Result<(), String> {
        let Point(x, y) = point;
        // If the x, y values go off the screen, wrap them
        let x = if *x < 0 {
            GRID_X_SIZE as i32 + *x
        } else if *x >= GRID_X_SIZE as i32 {
            *x - GRID_X_SIZE as i32
        } else {
            *x
        };
        let y = if *y < 0 {
            GRID_Y_SIZE as i32 + *y
        } else if *y >= GRID_Y_SIZE as i32 {
            *y - GRID_Y_SIZE as i32
        } else {
            *y
        };
        self.canvas.fill_rect(Rect::new(
            x * DOT_SIZE_IN_PXS as i32,
            y * DOT_SIZE_IN_PXS as i32,
            DOT_SIZE_IN_PXS,
            DOT_SIZE_IN_PXS,
        ))?;

        Ok(())
    }
    pub fn draw(&mut self, context: &EmulatorContext) -> Result<(), String> {
        self.draw_display(context)?;
        self.canvas.present();

        Ok(())
    }

    fn draw_display(&mut self, context: &EmulatorContext) -> Result<(), String> {
        for y in 0..GRID_Y_SIZE {
            for x in 0..GRID_X_SIZE {
                let point = Point(x as i32, y as i32);
                let color = if context.display.contains(&point) {
                    Color::WHITE
                } else {
                    Color::BLACK
                };
                self.canvas.set_draw_color(color);
                self.draw_dot(&point)?;
            }
        }

        Ok(())
    }
}

pub fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let audio_subsystem = sdl_context.audio()?;

    let window = video_subsystem
        .window(
            "CHIP-8 Emulator",
            GRID_X_SIZE * DOT_SIZE_IN_PXS,
            GRID_Y_SIZE * DOT_SIZE_IN_PXS,
        )
        .position_centered()
        .opengl()
        .build()
        .map_err(|e| e.to_string())?;

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: None,
    };

    let device = audio_subsystem.open_playback(None, &desired_spec, |spec| Speaker {
        phase_inc: 440.0 / spec.freq as f32,
        phase: 0.0,
        volume: 0.25,
    })?;
    let mut muted: bool = false;

    let mut context = EmulatorContext::new();
    let mut renderer = Renderer::new(window)?;

    context.load_sprites_into_memory();
    context.load_program_into_memory();

    renderer.draw(&context)?;

    let mut event_pump = sdl_context.event_pump()?;

    let mut speed: u32 = 1;
    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    Keycode::Space => context.toggle_pause(),
                    Keycode::M => muted = !muted,
                    Keycode::RightBracket => speed = if speed < 2 { speed + 1 } else { 1 },
                    _ if KEYMAP.iter().any(|&x| x.0 == keycode) => {
                        context.keyboard.key_down(keycode);
                    }
                    _ => {}
                },
                Event::KeyUp {
                    keycode: Some(keycode),
                    ..
                } => match keycode {
                    _ if KEYMAP.iter().any(|&x| x.0 == keycode) => {
                        context.keyboard.key_up(keycode);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        let sleep_duration = Duration::from_millis(1000 / 60);

        context.cycle();
        renderer.draw(&context)?;
        if context.sound_timer > 0 && !muted {
            device.resume();
        } else {
            device.pause();
        }

        ::std::thread::sleep(sleep_duration / speed);
    }

    Ok(())
}
