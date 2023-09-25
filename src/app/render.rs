use super::{display, io, sprite, state};
use specs::{Read, ReadStorage, System};

pub struct RenderBuffer {
    pub screen: display::Screen,
}

const HB_CHARS: &[char] = &['▀', '▄'];

#[allow(dead_code)]
fn render_bg_checkerboard(scr: &mut display::Screen) {
    let sz = {
        let scr_size = scr.size();
        (scr_size.width as u16, scr_size.height as u16)
    };

    let mut bg_style = display::Style::new();
    bg_style.set_fg(display::Color::Rgb {
        r: 10,
        g: 20,
        b: 60,
    });
    for y in 0..(sz.1) {
        let mut bg_char_index = 0;
        for x in 0..(sz.0) {
            scr.put(
                HB_CHARS[bg_char_index],
                bg_style,
                display::ScreenPos { x, y },
            );
            bg_char_index = if bg_char_index == 0 { 1 } else { 0 };
        }
    }
}

fn render_sprite_at_pos(
    scr: &mut display::Screen,
    info: &sprite::LoadedSprite,
    sprite: &state::Sprite,
    pos: &state::Position,
) {
    let sz = {
        let scr_size = scr.size();
        (scr_size.width as u16, scr_size.height as u16)
    };

    let active_frame_number = sprite.frame as usize;
    let active_frame_data = &info.data.frames[active_frame_number];
    let frame = &active_frame_data.frame;
    let image = &info.image;

    let flip = sprite.flip;
    let mut rows_vec: Vec<Vec<sprite::RGBA>> = vec![];
    for y in frame.y..(frame.y + frame.h) {
        let y = y as usize;
        let mut row_vec: Vec<sprite::RGBA> = vec![];
        if flip {
            for x in (frame.x..(frame.x + frame.w)).rev() {
                let x = x as usize;
                row_vec.push(image[y][x]);
            }
        } else {
            for x in frame.x..(frame.x + frame.w) {
                let x = x as usize;
                row_vec.push(image[y][x]);
            }
        }
        rows_vec.push(row_vec);
    }

    // iterate over rows, plotting unicode characters to scr.put
    'outer: for y in (0..(rows_vec.len() as usize)).step_by(2) {
        if (pos.y + y as i64) < 0 {
            continue 'outer;
        }
        let scr_y = pos.y as u16 + (y / 2) as u16;
        if scr_y > sz.1 - 1 {
            continue 'outer;
        }

        let row = &rows_vec[y];
        'inner: for x in 0..(row.len() as usize) {
            if (pos.x + x as i64) < 0 {
                continue 'inner;
            }
            let scr_x = pos.x as u16 + x as u16;
            if scr_x > sz.0 - 1 {
                continue 'inner;
            }
           
            // px2 is the pixel beneath px in the sprite image
            let px = row[x];
            let px2 = rows_vec[(y + 1) as usize][x as usize];

            let (mut r, mut g, mut b, a) = (px.r, px.g, px.b, px.a);
            let (mut r2, mut g2, mut b2, a2) = (px2.r, px2.g, px2.b, px2.a);

            // if highlighting this sprite, change black outlines to bright
            if sprite.highlight {
                if r == 0 && g == 0 && b == 0 && a > 0 {
                    (r, g, b) = (127, 255, 255);
                }
                if r2 == 0 && g2 == 0 && b2 == 0 && a2 > 0 {
                    (r2, g2, b2) = (127, 255, 255);
                }
            }

            // get the underlying pixel colors
            let (u1, u2) = {
                let black = display::Color::Rgb { r: 0, g: 0, b: 0 };
                let (mut u1, mut u2) = (black, black);
                let scr_get_result = scr.get(display::ScreenPos { x: scr_x, y: scr_y });
                if !scr_get_result.is_none() {
                    let under = scr_get_result.unwrap();
                    let ch = under.0;
                    let st = under.1;
                    if ch == HB_CHARS[0] {
                        u1 = st.fg.unwrap();
                        u2 = st.bg.unwrap();
                    } else if ch == HB_CHARS[1] {
                        u1 = st.bg.unwrap();
                        u2 = st.fg.unwrap();
                    } else {
                        u1 = display::Color::Rgb { r: px.r, g: px.g, b: px.b };
                        u2 = display::Color::Rgb { r: px2.r, g: px2.g, b: px2.b };
                    }
                }
                (u1, u2)
            };

            // draw pixels to screen positions using HB_CHARS
            // use style.set_fg and style.set_bg based on a, a2, etc
            let mut style = display::Style::new();
            if a > 0 {
                style.set_fg(display::Color::Rgb { r, g, b });
                if a2 > 0 {
                    style.set_bg(display::Color::Rgb {
                        r: r2,
                        g: g2,
                        b: b2,
                    });
                } else {
                    style.set_bg(u2);
                }
                scr.put(
                    HB_CHARS[0],
                    style,
                    display::ScreenPos { x: scr_x, y: scr_y },
                );
            } else {
                if a2 > 0 {
                    style.set_bg(u1);
                    style.set_fg(display::Color::Rgb {
                        r: r2,
                        g: g2,
                        b: b2,
                    });
                    scr.put(
                        HB_CHARS[1],
                        style,
                        display::ScreenPos { x: scr_x, y: scr_y },
                    );
                }
            }
        }
    }
}

fn render_text_at_pos(scr: &mut display::Screen, text: &str, start_x: u16, start_y: u16) {
    let sz = {
        let scr_size = scr.size();
        (scr_size.width, scr_size.height)
    };

    let mut style = display::Style::new();
    style.set_fg(display::Color::Rgb {
        r: 255,
        g: 255,
        b: 255,
    });
    style.set_bg(display::Color::Rgb { r: 0, g: 0, b: 95 });

    let (mut x, mut y) = (start_x, start_y);
    for ch in text.chars() {
        if x > sz.0 as u16 - 1 {
            break;
        }

        if ch == '\n' {
            y += 1;
            x = start_x;
            continue;
        }

        scr.put(ch, style, display::ScreenPos { x, y });
        x += 1;
    }
}

impl<'a> System<'a> for RenderBuffer {
    type SystemData = (
        Read<'a, state::Game>,
        Read<'a, sprite::SpriteStore>,
        ReadStorage<'a, state::Sprite>,
        ReadStorage<'a, state::Position>,
    );

    fn run(&mut self, data: Self::SystemData) {
        use specs::Join;

        let (game, store, sprites, positions) = data;
        let scr = &mut self.screen;

        if game.clear_screen {
            scr.clear_all(io::stdout()).expect("scr clear all error");
            scr.render(io::stdout()).expect("scr render error");
        } else {
            scr.erase();
        }

        let sz = {
            let scr_size = scr.size();
            (scr_size.width as u16, scr_size.height as u16)
        };
        
        // per-sprite debug information for debug builds
        struct DebugFrameNumber {
            x: i64,
            y: i64,
            num: String,
        }
        let mut debug_numbers: Vec<DebugFrameNumber> = vec![];
        
        // get sorted sprites by 'z' on position
        let mut sorted_sprites = (&positions, &sprites).join().collect::<Vec<_>>();
        sorted_sprites.sort_by(|a, b| {
            a.0.z
                .partial_cmp(&b.0.z)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (pos, sprite) in sorted_sprites.iter_mut() {
            let info = &store.0[sprite.store_index];
            render_sprite_at_pos(scr, info, sprite, pos);

            // collect debug information
            if cfg!(debug_assertions)
                && (sprite.sprite_type == state::SpriteType::Tool
                    || sprite.sprite_type == state::SpriteType::Crop)
            {
                let frame_number_string = format!("{}", sprite.frame);
                debug_numbers.push(DebugFrameNumber {
                    x: pos.x,
                    y: pos.y,
                    num: frame_number_string,
                    // num: sprite.id.to_string(),
                });
            }
        }

        if game.show_terminal {
            let no_messages = "No new messages.".to_string();
            let message = if game.terminal_read {
                &no_messages
            } else {
                &game.terminal_messages[game.terminal_message_index]
            };
            render_text_at_pos(scr, message.as_str(), 1, 0);
        }

        if game.show_help {
            let tooltip = "arrows/hjkl: move | q: quit | space: pickup | u: use | ?: hide help ";
            render_text_at_pos(scr, tooltip, 0, sz.1 - 1);

            if cfg!(debug_assertions) {
                for debug in debug_numbers {
                    render_text_at_pos(scr, debug.num.as_str(), debug.x as u16, debug.y as u16);
                }
            }
        }

        scr.render(io::stdout()).expect("scr render error");
    }
}
