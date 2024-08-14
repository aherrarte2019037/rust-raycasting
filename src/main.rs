#![allow(dead_code)]
use crate::player::{SideMovement, StraightMovement, TurnMovement};
use cache::Picture;
use clap::Parser;
use map::{Map, Tile};
use player::Player;
use core::slice::Iter;
use minifb::{Key, KeyRepeat, Window, WindowOptions};
use rodio::{source::Source, Decoder, OutputStream};
use std::fs::File;
use std::io::BufReader;
use std::time::{Duration, Instant};

mod cache;
type ColorMap = [(u8, u8, u8); 256];
mod constants;
mod map;
mod player;
mod ray_caster;

use constants::*;

const VGA_FLOOR_COLOR: usize = 0x19;
const VGA_CEILING_COLORS: [usize; 60] = [
    0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0xbf, 0x4e, 0x4e, 0x4e, 0x1d, 0x8d, 0x4e,
    0x1d, 0x2d, 0x1d, 0x8d, 0x1d, 0x1d, 0x1d, 0x1d, 0x1d, 0x2d, 0xdd, 0x1d, 0x1d, 0x98, 0x1d, 0x9d,
    0x2d, 0xdd, 0xdd, 0x9d, 0x2d, 0x4d, 0x1d, 0xdd, 0x7d, 0x1d, 0x2d, 0x2d, 0xdd, 0xd7, 0x1d, 0x1d,
    0x1d, 0x2d, 0x1d, 0x1d, 0x1d, 0x1d, 0xdd, 0xdd, 0x7d, 0xdd, 0xdd, 0xdd,
];

const DARKNESS: f64 = 0.75;

#[derive(Parser, Debug)]
struct Opts {
    #[clap(short, long, default_value="3", possible_values=["1","2","3","4","5"])]
    scale: u32,

    #[clap(short, long, default_value="0", possible_values=["0", "1","2","3"])]
    dificulty: usize,

    #[clap(short, long, default_value="1", possible_values=["1","2","3","4","5","6","7","8","9","10"])]
    level: usize,
}

struct Video {
    pub width: u32,
    pub height: u32,
    pub pix_width: u32,
    pub pix_height: u32,
    pub pix_center: u32,
    pub scale: u32,
    pub color_map: ColorMap,
    pub buffer: Vec<u32>,
}

struct Game {
    player: player::Player,
    map: map::Map,
    episode: usize,
    level: usize,
    start_time: Instant,
    cache: cache::Cache,
}

pub fn main() {
    let args = Opts::parse();
    let mut game = Game::new(args.level);
    let mut video = Video::new(args.scale);

    let mut window = Window::new(
        "Rust Raycasting",
        video.width as usize,
        video.height as usize,
        WindowOptions::default(),
    )
    .unwrap();

    window.limit_update_rate(Some(std::time::Duration::from_micros(16600)));

    show_title(&game, &mut video, &mut window);
    let map = &game.map;

    let mut last_time = Instant::now();
    let mut frame_count = 0;
    let mut fps = 0;

    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let file = BufReader::new(File::open("data/background-music.ogg").unwrap());
    let source = Decoder::new(file).unwrap();
    let _ = stream_handle.play_raw(source.convert_samples());
    std::thread::sleep(std::time::Duration::from_millis(100));

    while process_input(&window, &mut game.player, map).is_ok() {
        let now = Instant::now();
        frame_count += 1;

        if now.duration_since(last_time) >= Duration::from_secs(1) {
            fps = frame_count;
            frame_count = 0;
            last_time = now;
        }

        draw_world(&game, &mut video);
        draw_weapon(&game, &mut video);

        video.draw_minimap(&game.map, &game.player, 2);
        video.draw_fps_counter(fps);
        video.present(&mut window);
    }
}

fn process_input(
    window: &Window,
    player: &mut player::Player,
    map: &map::Map,
) -> Result<(), String> {
    if !window.is_open() || window.is_key_pressed(Key::Escape, KeyRepeat::No) {
        return Err(String::from("Goodbye!"));
    }

    let mut straight: Option<StraightMovement> = None;
    let mut side: Option<SideMovement> = None;
    let mut turn: Option<TurnMovement> = None;
    let mut run = false;
    if window.is_key_down(Key::LeftShift) {
        run = true;
    }

    if window.is_key_down(Key::Left) || window.is_key_down(Key::A) {
        if window.is_key_down(Key::X) {
            side = Some(SideMovement::StrafeLeft);
        } else {
            turn = Some(TurnMovement::TurnLeft);
        }
    }

    if window.is_key_down(Key::Right) || window.is_key_down(Key::D) {
        if window.is_key_down(Key::X) {
            side = Some(SideMovement::StrafeRight);
        } else {
            turn = Some(TurnMovement::TurnRight);
        }
    }

    if window.is_key_down(Key::Up) || window.is_key_down(Key::W) {
        straight = Some(StraightMovement::Forward);
    }

    if window.is_key_down(Key::Down) || window.is_key_down(Key::S) {
        straight = Some(StraightMovement::Backward);
    }

    if window.is_key_down(Key::Q) {
        side = Some(SideMovement::StrafeLeft);
    }

    if window.is_key_down(Key::E) {
        side = Some(SideMovement::StrafeRight);
    }

    player.walk(map, straight, side, turn, run);

    Ok(())
}

fn show_title(game: &Game, video: &mut Video, window: &mut Window) {
    let titlepic = game.cache.get_pic(cache::TITLEPIC);
    video.draw_texture(0, 0, titlepic);

    while window.get_keys_pressed(KeyRepeat::No).is_empty() {
        video.present(window);
    }
}

fn draw_world(game: &Game, video: &mut Video) {
    let ray_hits =
        ray_caster::draw_rays(video.pix_width, video.pix_height, &game.map, &game.player);

    for x in 0..video.pix_width {
        for y in 0..video.pix_height / 2 {
            video.put_darkened_pixel(x, y, VGA_CEILING_COLORS[game.level], video.pix_center - y);
        }
        for y in video.pix_height / 2..video.pix_height {
            video.put_darkened_pixel(x, y, VGA_FLOOR_COLOR, y - video.pix_center);
        }
    }

    for x in 0..video.pix_width {
        let hit = &ray_hits[x as usize];

        let wallpic = if hit.horizontal {
            (hit.tile - 1) * 2
        } else {
            (hit.tile - 1) * 2 + 1
        };
        let texture = game.cache.get_texture(wallpic as usize);

        let current = ray_hits[x as usize].height as i32;
        let xoff = hit.tex_x * WALLPIC_WIDTH;

        let step = WALLPIC_WIDTH as f64 / 2.0 / current as f64;
        let mut ytex = 0.0;

        for y in video.pix_center as i32 - current..video.pix_center as i32 + current {
            if y >= 0 && y <= video.pix_height as i32 {
                let source = ytex as usize + xoff;
                let color_index = texture[source] as usize;

                video.put_darkened_pixel(x, y as u32, color_index, current as u32);
            }

            ytex += step;
        }
    }
}

fn draw_weapon(game: &Game, video: &mut Video) {
    let (weapon_shape, weapon_data) = game.cache.get_sprite(209);

    video.simple_scale_shape(
        weapon_shape.left_pix,
        weapon_shape.right_pix,
        &weapon_shape.dataofs,
        weapon_data,
    );
}

impl Game {
    pub fn new(level: usize) -> Self {
        let level = level - 1;
        let cache = cache::init();
        let map = cache.get_map(0, level);
        let player = map.find_player();
        Self {
            cache,
            map,
            player,
            episode: 0,
            level,
            start_time: Instant::now(),
        }
    }
}

impl Video {
    pub fn new(scale: u32) -> Self {
        let width = BASE_WIDTH * scale;
        let height = BASE_HEIGHT * scale;
        let pix_width = width;
        let pix_height = height - STATUS_LINES * scale;
        let pix_center = pix_height / 2;
        let buffer: Vec<u32> = vec![0; (width * height) as usize];

        Self {
            scale,
            width,
            height,
            pix_width,
            pix_height,
            pix_center,
            color_map: build_color_map(),
            buffer,
        }
    }

    pub fn put_pixel(&mut self, x: u32, y: u32, color_index: usize) {
        if x >= self.width || y >= self.height {
            return;
        }

        if color_index >= self.color_map.len() {
            return;
        }

        let offset = (y * self.width + x) as usize;

        if offset < self.buffer.len() {
            let (r, g, b) = self.color_map[color_index];
            let (r, g, b) = (r as u32, g as u32, b as u32);

            self.buffer[offset] = (r << 16) | (g << 8) | b;
        }
    }

    pub fn put_darkened_pixel(&mut self, x: u32, y: u32, color_index: usize, lightness: u32) {
        if x >= self.width || y >= self.height {
            return;
        }

        let offset = (y * self.width + x) as usize;

        if offset >= self.buffer.len() {
            return;
        }

        let (r, g, b) = self.color_map[color_index as usize];

        let factor =
            std::cmp::min(lightness, self.pix_center) as f64 / self.pix_center as f64 / DARKNESS;
        let r = (r as f64 * factor) as u8 as u32;
        let g = (g as f64 * factor) as u8 as u32;
        let b = (b as f64 * factor) as u8 as u32;

        self.buffer[offset] = (r << 16) | (g << 8) | b;
    }

    pub fn present(&self, window: &mut Window) {
        window
            .update_with_buffer(&self.buffer, self.width as usize, self.height as usize)
            .unwrap();
    }

    pub fn draw_texture(&mut self, shift_x: u32, shift_y: u32, pic: &Picture) {
        let mut scj = 0;
        for y in 0..pic.height {
            let mut sci = 0;
            for x in 0..pic.width {
                let source_index =
                    (y * (pic.width >> 2) + (x >> 2)) + (x & 3) * (pic.width >> 2) * pic.height;
                let color = pic.data[source_index as usize];
                for i in 0..self.scale {
                    for j in 0..self.scale {
                        self.put_pixel(sci + j + shift_x, scj + i + shift_y, color as usize);
                    }
                }

                sci += self.scale
            }
            scj += self.scale
        }
    }

    fn simple_scale_shape(
        &mut self,
        left_pix: u16,
        right_pix: u16,
        dataofs: &[u16],
        shape_bytes: &[u8],
    ) {
        let sprite_scale_factor = 2;
        let xcenter = self.pix_width / 2;
        let height = self.pix_height + 1;

        let scale = height >> 1;
        let pixheight = scale * sprite_scale_factor;
        let actx = xcenter - scale;
        let upperedge = self.pix_height / 2 - scale;
        let mut cmdptr = dataofs.iter();

        let mut i = left_pix;
        let mut pixcnt = i as u32 * pixheight;
        let mut rpix = (pixcnt >> 6) + actx;

        while i <= right_pix {
            let mut lpix = rpix;
            if lpix >= self.pix_width {
                break;
            }

            pixcnt += pixheight;
            rpix = (pixcnt >> 6) + actx;

            if lpix != rpix && rpix > 0 {
                if rpix > self.pix_width {
                    rpix = self.pix_width;
                    i = right_pix + 1;
                }
                let read_word = |line: &mut Iter<u8>| {
                    u16::from_le_bytes([*line.next().unwrap_or(&0), *line.next().unwrap_or(&0)])
                };
                let read_word_signed = |line: &mut Iter<u8>| {
                    i16::from_le_bytes([*line.next().unwrap_or(&0), *line.next().unwrap_or(&0)])
                };

                let cline = &shape_bytes[*cmdptr.next().unwrap() as usize..];
                while lpix < rpix {
                    let mut line = cline.iter();
                    let mut endy = read_word(&mut line);
                    while endy > 0 {
                        endy >>= 1;
                        let newstart = read_word_signed(&mut line);
                        let starty = read_word(&mut line) >> 1;
                        let mut j = starty;
                        let mut ycnt = j as u32 * pixheight;
                        let mut screndy: i32 = (ycnt >> 6) as i32 + upperedge as i32;

                        let mut pixy = screndy as u32;
                        while j < endy {
                            let mut scrstarty = screndy;
                            ycnt += pixheight;
                            screndy = (ycnt >> 6) as i32 + upperedge as i32;
                            if scrstarty != screndy && screndy > 0 {
                                let index = newstart + j as i16;
                                let col = if index >= 0 {
                                    shape_bytes[index as usize]
                                } else {
                                    0
                                };
                                if scrstarty < 0 {
                                    scrstarty = 0;
                                }
                                if screndy > self.pix_height as i32 {
                                    screndy = self.pix_height as i32;
                                    j = endy;
                                }

                                while scrstarty < screndy {
                                    self.put_pixel(lpix, pixy, col as usize);
                                    pixy += 1;
                                    scrstarty += 1;
                                }
                            }
                            j += 1;
                        }
                        endy = read_word(&mut line);
                    }
                    lpix += 1;
                }
            }
            i += 1;
        }
    }

    pub fn draw_fps_counter(&mut self, fps: usize) {
        let x = 5;
        let y = 5;
        let scale = 2;

        for (i, digit) in fps.to_string().chars().enumerate() {
            self.draw_digit(x + i as u32 * 4 * scale, y, digit as u8 - '0' as u8, scale);
        }
    }

    pub fn draw_digit(&mut self, x: u32, y: u32, digit: u8, scale: u32) {
        const DIGITS: [[u8; 5]; 10] = [
            [0b111, 0b101, 0b101, 0b101, 0b111],
            [0b010, 0b110, 0b010, 0b010, 0b111],
            [0b111, 0b001, 0b111, 0b100, 0b111],
            [0b111, 0b001, 0b111, 0b001, 0b111],
            [0b101, 0b101, 0b111, 0b001, 0b001],
            [0b111, 0b100, 0b111, 0b001, 0b111],
            [0b111, 0b100, 0b111, 0b101, 0b111],
            [0b111, 0b001, 0b001, 0b001, 0b001],
            [0b111, 0b101, 0b111, 0b101, 0b111],
            [0b111, 0b101, 0b111, 0b001, 0b111],
        ];

        if digit > 9 {
            return;
        }

        let pattern = DIGITS[digit as usize];

        for (dy, row) in pattern.iter().enumerate() {
            for dx in 0..3 {
                if row & (1 << (2 - dx)) != 0 {
                    for sy in 0..scale {
                        for sx in 0..scale {
                            self.put_pixel(
                                x + dx as u32 * scale + sx,
                                y + dy as u32 * scale + sy,
                                255,
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn draw_minimap(&mut self, map: &Map, player: &Player, minimap_scale: u32) {
        let minimap_size = 128;
        let map_width = MAP_WIDTH as u32;
        let map_height = MAP_HEIGHT as u32;

        let minimap_x = self.width - minimap_size - 10;
        let minimap_y = 10;

        for y in 0..map_height {
            for x in 0..map_width {
                let tile = map.tile_at(x as u8, y as u8);
                let color_index = match tile {
                    Tile::Wall(_) => 255,
                    Tile::Floor | Tile::Door { .. } => 0,
                };

                let screen_x = minimap_x + x * minimap_scale;
                let screen_y = minimap_y + y * minimap_scale;

                for i in 0..minimap_scale {
                    for j in 0..minimap_scale {
                        self.put_pixel(screen_x + i, screen_y + j, color_index);
                    }
                }
            }
        }

        let player_x = player.x as u32 / MAP_SCALE_W * minimap_scale + minimap_x;
        let player_y = player.y as u32 / MAP_SCALE_H * minimap_scale + minimap_y;

        let player_color_index = 10;

        for i in 0..minimap_scale {
            for j in 0..minimap_scale {
                self.put_pixel(player_x + i, player_y + j, player_color_index);
            }
        }
    }
}

fn build_color_map() -> ColorMap {
    let palette = [
        (0, 0, 0),
        (0, 0, 42),
        (0, 42, 0),
        (0, 42, 42),
        (42, 0, 0),
        (42, 0, 42),
        (42, 21, 0),
        (42, 42, 42),
        (21, 21, 21),
        (21, 21, 63),
        (21, 63, 21),
        (21, 63, 63),
        (63, 21, 21),
        (63, 21, 63),
        (63, 63, 21),
        (63, 63, 63),
        (59, 59, 59),
        (55, 55, 55),
        (52, 52, 52),
        (48, 48, 48),
        (45, 45, 45),
        (42, 42, 42),
        (38, 38, 38),
        (35, 35, 35),
        (31, 31, 31),
        (28, 28, 28),
        (25, 25, 25),
        (21, 21, 21),
        (18, 18, 18),
        (14, 14, 14),
        (11, 11, 11),
        (8, 8, 8),
        (63, 0, 0),
        (59, 0, 0),
        (56, 0, 0),
        (53, 0, 0),
        (50, 0, 0),
        (47, 0, 0),
        (44, 0, 0),
        (41, 0, 0),
        (38, 0, 0),
        (34, 0, 0),
        (31, 0, 0),
        (28, 0, 0),
        (25, 0, 0),
        (22, 0, 0),
        (19, 0, 0),
        (16, 0, 0),
        (63, 54, 54),
        (63, 46, 46),
        (63, 39, 39),
        (63, 31, 31),
        (63, 23, 23),
        (63, 16, 16),
        (63, 8, 8),
        (63, 0, 0),
        (63, 42, 23),
        (63, 38, 16),
        (63, 34, 8),
        (63, 30, 0),
        (57, 27, 0),
        (51, 24, 0),
        (45, 21, 0),
        (39, 19, 0),
        (63, 63, 54),
        (63, 63, 46),
        (63, 63, 39),
        (63, 63, 31),
        (63, 62, 23),
        (63, 61, 16),
        (63, 61, 8),
        (63, 61, 0),
        (57, 54, 0),
        (51, 49, 0),
        (45, 43, 0),
        (39, 39, 0),
        (33, 33, 0),
        (28, 27, 0),
        (22, 21, 0),
        (16, 16, 0),
        (52, 63, 23),
        (49, 63, 16),
        (45, 63, 8),
        (40, 63, 0),
        (36, 57, 0),
        (32, 51, 0),
        (29, 45, 0),
        (24, 39, 0),
        (54, 63, 54),
        (47, 63, 46),
        (39, 63, 39),
        (32, 63, 31),
        (24, 63, 23),
        (16, 63, 16),
        (8, 63, 8),
        (0, 63, 0),
        (0, 63, 0),
        (0, 59, 0),
        (0, 56, 0),
        (0, 53, 0),
        (1, 50, 0),
        (1, 47, 0),
        (1, 44, 0),
        (1, 41, 0),
        (1, 38, 0),
        (1, 34, 0),
        (1, 31, 0),
        (1, 28, 0),
        (1, 25, 0),
        (1, 22, 0),
        (1, 19, 0),
        (1, 16, 0),
        (54, 63, 63),
        (46, 63, 63),
        (39, 63, 63),
        (31, 63, 62),
        (23, 63, 63),
        (16, 63, 63),
        (8, 63, 63),
        (0, 63, 63),
        (0, 57, 57),
        (0, 51, 51),
        (0, 45, 45),
        (0, 39, 39),
        (0, 33, 33),
        (0, 28, 28),
        (0, 22, 22),
        (0, 16, 16),
        (23, 47, 63),
        (16, 44, 63),
        (8, 42, 63),
        (0, 39, 63),
        (0, 35, 57),
        (0, 31, 51),
        (0, 27, 45),
        (0, 23, 39),
        (54, 54, 63),
        (46, 47, 63),
        (39, 39, 63),
        (31, 32, 63),
        (23, 24, 63),
        (16, 16, 63),
        (8, 9, 63),
        (0, 1, 63),
        (0, 0, 63),
        (0, 0, 59),
        (0, 0, 56),
        (0, 0, 53),
        (0, 0, 50),
        (0, 0, 47),
        (0, 0, 44),
        (0, 0, 41),
        (0, 0, 38),
        (0, 0, 34),
        (0, 0, 31),
        (0, 0, 28),
        (0, 0, 25),
        (0, 0, 22),
        (0, 0, 19),
        (0, 0, 16),
        (10, 10, 10),
        (63, 56, 13),
        (63, 53, 9),
        (63, 51, 6),
        (63, 48, 2),
        (63, 45, 0),
        (45, 8, 63),
        (42, 0, 63),
        (38, 0, 57),
        (32, 0, 51),
        (29, 0, 45),
        (24, 0, 39),
        (20, 0, 33),
        (17, 0, 28),
        (13, 0, 22),
        (10, 0, 16),
        (63, 54, 63),
        (63, 46, 63),
        (63, 39, 63),
        (63, 31, 63),
        (63, 23, 63),
        (63, 16, 63),
        (63, 8, 63),
        (63, 0, 63),
        (56, 0, 57),
        (50, 0, 51),
        (45, 0, 45),
        (39, 0, 39),
        (33, 0, 33),
        (27, 0, 28),
        (22, 0, 22),
        (16, 0, 16),
        (63, 58, 55),
        (63, 56, 52),
        (63, 54, 49),
        (63, 53, 47),
        (63, 51, 44),
        (63, 49, 41),
        (63, 47, 39),
        (63, 46, 36),
        (63, 44, 32),
        (63, 41, 28),
        (63, 39, 24),
        (60, 37, 23),
        (58, 35, 22),
        (55, 34, 21),
        (52, 32, 20),
        (50, 31, 19),
        (47, 30, 18),
        (45, 28, 17),
        (42, 26, 16),
        (40, 25, 15),
        (39, 24, 14),
        (36, 23, 13),
        (34, 22, 12),
        (32, 20, 11),
        (29, 19, 10),
        (27, 18, 9),
        (23, 16, 8),
        (21, 15, 7),
        (18, 14, 6),
        (16, 12, 6),
        (14, 11, 5),
        (10, 8, 3),
        (24, 0, 25),
        (0, 25, 25),
        (0, 24, 24),
        (0, 0, 7),
        (0, 0, 11),
        (12, 9, 4),
        (18, 0, 18),
        (20, 0, 20),
        (0, 0, 13),
        (7, 7, 7),
        (19, 19, 19),
        (23, 23, 23),
        (16, 16, 16),
        (12, 12, 12),
        (13, 13, 13),
        (54, 61, 61),
        (46, 58, 58),
        (39, 55, 55),
        (29, 50, 50),
        (18, 48, 48),
        (8, 45, 45),
        (8, 44, 44),
        (0, 41, 41),
        (0, 38, 38),
        (0, 35, 35),
        (0, 33, 33),
        (0, 31, 31),
        (0, 30, 30),
        (0, 29, 29),
        (0, 28, 28),
        (0, 27, 27),
        (38, 0, 34),
    ];
    palette.map(|(r, g, b)| {
        (
            (r * 255 / 63) as u8,
            (g * 255 / 63) as u8,
            (b * 255 / 63) as u8,
        )
    })
}
