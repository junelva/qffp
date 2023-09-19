use rand::Rng;
use std::io::{self, Write as IOWrite};
use std::path;
use std::time::SystemTime;
use std::{str, time::Duration};

use crossterm::event::{poll, read, Event, KeyCode, KeyModifiers};
use crossterm::{cursor, event, style, terminal, QueueableCommand};
use specs::{Builder, Dispatcher, DispatcherBuilder, World, WorldExt};
use thiserror::Error;

// using anathema's display module for double buffered screen output
use anathema::display;

mod render;
mod sprite;
mod state;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("stdio error")]
    StdioError(#[from] std::io::Error),
    #[error("utf8 error")]
    Utf8Error(#[from] str::Utf8Error),
    #[error("serde_json error")]
    SerdeError(#[from] serde_json::Error),
    #[error("image error")]
    ImageError(#[from] image::ImageError),
    #[error("spritestore error")]
    SpriteStoreError(#[from] sprite::SpriteStoreError),
}

#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub enum InputState {
    Up,
    Down,
    Left,
    Right,
    ShiftUp,
    ShiftDown,
    ShiftLeft,
    ShiftRight,
    Pickup,
    Action,
    ToggleHelp,
    Quit,
    None,
}

pub struct Input(pub InputState);

impl Default for Input {
    fn default() -> Self {
        Input(InputState::None)
    }
}

pub struct App<'a> {
    time_start: SystemTime,
    time: u64,
    world: World,
    dispatcher: Dispatcher<'a, 'a>,
    pub input: InputState,
}

impl<'a> App<'a> {
    /// gather key input. 'ms' is the poll duration in milliseconds.
    pub fn process_input(&mut self, ms: u64) -> Result<InputState, AppError> {
        let mut input: InputState = InputState::None;
        if poll(Duration::from_millis(ms))? {
            match read()? {
                Event::FocusGained => {}
                Event::FocusLost => {}
                Event::Key(event) => 'key: {
                    if event.kind != event::KeyEventKind::Press {
                        break 'key;
                    }
                    let code = event.code;
                    let mods = event.modifiers;

                    // quit command
                    if (code == KeyCode::Char('c') && mods == KeyModifiers::CONTROL)
                        || code == KeyCode::Char('q')
                    {
                        input = InputState::Quit;
                    }
                    // movement commands
                    else if code == KeyCode::Left
                        || code == KeyCode::Char('h')
                        || code == KeyCode::Char('H')
                    {
                        if mods == KeyModifiers::SHIFT {
                            input = InputState::ShiftLeft;
                        } else {
                            input = InputState::Left;
                        }
                    } else if code == KeyCode::Right
                        || code == KeyCode::Char('l')
                        || code == KeyCode::Char('L')
                    {
                        if mods == KeyModifiers::SHIFT {
                            input = InputState::ShiftRight;
                        } else {
                            input = InputState::Right;
                        }
                    } else if code == KeyCode::Down
                        || code == KeyCode::Char('j')
                        || code == KeyCode::Char('J')
                    {
                        if mods == KeyModifiers::SHIFT {
                            input = InputState::ShiftDown;
                        } else {
                            input = InputState::Down;
                        }
                    } else if code == KeyCode::Up
                        || code == KeyCode::Char('k')
                        || code == KeyCode::Char('K')
                    {
                        if mods == KeyModifiers::SHIFT {
                            input = InputState::ShiftUp;
                        } else {
                            input = InputState::Up;
                        }
                    }
                    // other commands
                    else if code == KeyCode::Char('u') {
                        input = InputState::Action;
                    } else if code == KeyCode::Char(' ') {
                        input = InputState::Pickup;
                    } else if code == KeyCode::Char('?') {
                        input = InputState::ToggleHelp;
                    } else {
                        input = InputState::None;
                    }
                }
                Event::Mouse(_event) => {}
                Event::Paste(_data) => {}
                Event::Resize(_width, _height) => {}
            }
        }
        self.input = input;
        Ok(input)
    }

    // create a new App instance.
    pub fn new() -> Result<App<'a>, AppError> {
        let sz = terminal::size()?;

        // create initial app and register specs systems
        let mut app = App {
            time_start: SystemTime::now(),
            time: 0,
            world: World::new(),
            dispatcher: DispatcherBuilder::new()
                .with(state::UpdateGameState, "game_state", &[])
                .with(
                    render::RenderBuffer {
                        screen: display::Screen::new(io::stdout(), sz)?,
                    },
                    "render_buffer",
                    &["game_state"],
                )
                .build(),
            input: InputState::None,
        };

        // register specs componets
        app.world.register::<state::Sprite>();
        app.world.register::<state::Interactible>();
        app.world.register::<state::Position>();
        app.world.register::<state::NPC>();

        // insert specs resources
        app.world.insert(state::SpriteIndexer(0));
        app.world.insert(state::Time(0));
        app.world.insert(Input(InputState::None));

        // initialize sprite store with all sprite content
        let store = sprite::SpriteStore::new(vec![
            "res/sheets/character-00.json",
            "res/sheets/character-01.json",
            "res/sheets/tile-dirt.json",
            "res/sheets/grass.json",
            "res/sheets/tool-shovel.json",
            "res/sheets/tool-watercan.json",
            "res/sheets/tool-packet.json",
            "res/sheets/tool-packet2.json",
            "res/sheets/crop-empty.json",
            "res/sheets/crop-leaf.json",
            "res/sheets/crop-flower.json",
            "res/sheets/cryopod.json",
            "res/sheets/terminal.json",
            "res/sheets/transition.json",
            "res/sheets/particle-dirt.json",
            "res/sheets/particle-water.json",
            "res/sheets/particle-heart.json",
        ])?;

        let messages = vec![
            "### Welcome to Luna!\nYou've chosen to farm. Feel free to get started.\nYou will find a shovel, watercan, and seed packet nearby.\nPress 'u' again to mark this message as read and proceed.".to_string(),
            "### Keep up the good work.\nIf you water your crops,\nthey'll grow every day.".to_string(),
            "New message...\n... > Hey babe! I'll be there soon!\nI can't wait to see your farm. And face. -K".to_string(),
            "New message...\n... > Hey, I left you something.\nTry planting the seeds. -K".to_string(),
            "### Unauthorized crops detected.\nCease illegal personal growth immediately,\nor face farming license revocation.".to_string(),
            "New message...\n... > Aw, babe... you're actually growing them.\nRemember when we designed these crops together? -K".to_string(),
            "### Crop authorization granted.\nApologies for our mistake, doctor.\nThe AI responsible has been sacked.".to_string(),
            "New message...\n... > Okay, good news.\nDon't ask how, but... I'll be there tomorrow!\nGrow anything nice yet? -K".to_string(),
            "Special message intercepted...\n... > Hey, it's June. I hope you liked the demo.\nLove, peace, and pleasant farming to all who play this.\nWhatever you're struggling with, I believe in you.\nKeep up the good fight and we'll get through this together!".to_string(),
            "### Farming sequence completed. Have fun!".to_string(),
        ];

        app.world.insert(state::Game::new(messages));

        // initialize sprite indexer to give ids to sprites
        let mut si = state::SpriteIndexer(0);

        // spawn dirt and grass sprites
        let mut rng = rand::thread_rng();
        let dirt_frame_count = store.0[store.index_by_name("tile-dirt")?].data.frames.len();
        let grass_frame_count = store.0[store.index_by_name("grass")?].data.frames.len();
        for y in (0..sz.1).step_by(4) {
            for x in (0..(sz.0)).step_by(8) {
                let x = x as i64;
                let y = y as i64;

                // dirt tiles
                app.world
                    .create_entity()
                    .with(state::Sprite {
                        store_index: store.index_by_name("tile-dirt")?,
                        frame: rng.gen_range(0..dirt_frame_count),
                        flip: rng.gen_range(0..2) == 0,
                        ..state::Sprite::default()
                    })
                    .with(state::Position {
                        x,
                        y,
                        z: state::DEPTHS.ground as i64,
                    })
                    .build();

                if x % 16 == 0 && y % 8 == 0 {
                    app.world
                        .create_entity()
                        .with(state::Sprite {
                            store_index: store.index_by_name("transition")?,
                            sprite_type: state::SpriteType::Overlay,
                            animating: true,
                            ..state::Sprite::default()
                        })
                        .with(state::Position {
                            x,
                            y,
                            z: state::DEPTHS.overlay as i64,
                        })
                        .build();
                }

                // grass tiles have a chance to spawn
                if x < (sz.0 - 8).into() && y < (sz.1 - 4).into() && rng.gen_range(0..4) == 0 {
                    let id = si.new_index();
                    app.world
                        .create_entity()
                        .with(state::Sprite {
                            id, // grass has an id because it's interactive
                            store_index: store.index_by_name("grass")?,
                            frame: rng.gen_range(0..grass_frame_count),
                            flip: rng.gen_range(0..2) == 0,
                            animating: true,
                            sprite_type: state::SpriteType::Crop,
                            ..state::Sprite::default()
                        })
                        .with(state::Position {
                            x,
                            y,
                            z: state::DEPTHS.grass + id as i64,
                        })
                        .with(state::Interactible {
                            item_type: state::ItemType::Grass,
                            hold_to_use: false,
                        })
                        .build();
                }
            }
        }

        // spawn cryopod
        let mut id = si.new_index();
        app.world
            .create_entity()
            .with(state::Sprite {
                id,
                store_index: store.index_by_name("cryopod")?,
                animating: true,
                sprite_type: state::SpriteType::Tool,
                ..state::Sprite::default()
            })
            .with(state::Position {
                x: 1,
                y: 0,
                z: state::DEPTHS.player - id as i64,
            })
            .with(state::Interactible {
                item_type: state::ItemType::Pod,
                hold_to_use: false,
            })
            .build();

        // spawn terminal
        id = si.new_index();
        app.world
            .create_entity()
            .with(state::Sprite {
                id,
                store_index: store.index_by_name("terminal")?,
                sprite_type: state::SpriteType::Tool,
                animating: true,
                flip: true,
                ..state::Sprite::default()
            })
            .with(state::Position {
                x: 11,
                y: 2,
                z: state::DEPTHS.player - id as i64,
            })
            .with(state::Interactible {
                item_type: state::ItemType::Terminal,
                hold_to_use: false,
            })
            .build();

        // spawn player
        id = si.new_index();
        app.world
            .create_entity()
            .with(state::Sprite {
                id,
                store_index: store.index_by_name("character-00")?,
                sprite_type: state::SpriteType::Player,
                ..state::Sprite::default()
            })
            .with(state::Position {
                x: 5,
                y: (sz.1 as i64 / 2) - 2,
                z: state::DEPTHS.player + id as i64,
            })
            .build();
        
        // npc from start, for testing
        // app.world
        //     .create_entity()
        //     .with(state::Sprite {
        //         id,
        //         store_index: store.index_by_name("character-01")?,
        //         sprite_type: state::SpriteType::Tool,
        //         ..state::Sprite::default()
        //     })
        //     .with(state::Position {
        //         x: sz.0 as i64 / 2,
        //         y: (sz.1 as i64 / 2),
        //         z: state::DEPTHS.player + id as i64,
        //     })
        //     .with(state::Interactible {
        //         item_type: state::ItemType::NPC,
        //         hold_to_use: false,
        //     })
        //     .with(state::NPC {
        //         move_target: (sz.0 as i64 / 2 + 1, sz.1 as i64 / 2),
        //         last_move: 0,
        //         move_wait: 200,
        //         move_stop: 2000,
        //     })
        //     .build();

        // spawn tools
        id = si.new_index();
        app.world
            .create_entity()
            .with(state::Sprite {
                id,
                store_index: store.index_by_name("tool-shovel")?,
                sprite_type: state::SpriteType::Tool,
                ..state::Sprite::default()
            })
            .with(state::Position {
                x: 2,
                y: sz.1 as i64 - 7,
                z: state::DEPTHS.tools + id as i64,
            })
            .with(state::Interactible {
                item_type: state::ItemType::Shovel,
                hold_to_use: true,
            })
            .build();

        id = si.new_index();
        app.world
            .create_entity()
            .with(state::Sprite {
                id,
                store_index: store.index_by_name("tool-packet")?,
                sprite_type: state::SpriteType::Tool,
                ..state::Sprite::default()
            })
            .with(state::Position {
                x: 8,
                y: sz.1 as i64 - 4,
                z: state::DEPTHS.tools + id as i64,
            })
            .with(state::Interactible {
                item_type: state::ItemType::Packet,
                hold_to_use: true,
            })
            .build();

        id = si.new_index();
        app.world
            .create_entity()
            .with(state::Sprite {
                id,
                store_index: store.index_by_name("tool-watercan")?,
                sprite_type: state::SpriteType::Tool,
                ..state::Sprite::default()
            })
            .with(state::Position {
                x: 8,
                y: sz.1 as i64 - 8,
                z: state::DEPTHS.tools + id as i64,
            })
            .with(state::Interactible {
                item_type: state::ItemType::Watercan,
                hold_to_use: true,
            })
            .build();

        // we insert store and sprite indexer late because they're borrowed during sprite initialization
        app.world.insert(store);
        app.world.insert(si);

        // initialize crossterm settings
        terminal::enable_raw_mode()?;
        io::stdout()
            .queue(terminal::EnterAlternateScreen)?
            .queue(cursor::Hide)?
            .queue(cursor::SavePosition)?
            .queue(event::EnableMouseCapture)?
            .queue(terminal::Clear(terminal::ClearType::All))?
            .flush()?;
        Ok(app)
    }

    /// can be used to print debug info at the app level
    #[allow(dead_code)]
    pub fn debug_print(&mut self, text: &str) -> Result<(), AppError> {
        io::stdout()
            .queue(cursor::MoveTo(0, 0))?
            .queue(style::Print(text))?;
        Ok(())
    }

    /// update time resources used for input and animation
    fn update_time(&mut self) -> Result<(), AppError> {
        let new_time = SystemTime::now()
            .duration_since(self.time_start)
            .unwrap()
            .as_millis() as u64;

        {
            let mut time = self.world.write_resource::<state::Time>();
            *time = state::Time(new_time);
        }

        self.time = new_time;
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), AppError> {
        self.update_time()?;

        {
            let mut input = self.world.write_resource::<Input>();
            *input = Input(self.input);
        }

        self.dispatcher.dispatch(&mut self.world);
        self.world.maintain();

        self.input = InputState::None;
        Ok(())
    }

    pub fn exit(&self) -> Result<(), AppError> {
        terminal::disable_raw_mode()?;
        io::stdout()
            .queue(event::DisableMouseCapture)?
            .queue(cursor::RestorePosition)?
            .queue(cursor::Show)?
            .queue(terminal::LeaveAlternateScreen)?
            .flush()?;

        Ok(())
    }
}
