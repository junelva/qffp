use super::terminal;
use specs::{
    Component, Entities, LazyUpdate, Read, ReadStorage, System, VecStorage, Write, WriteStorage,
};
use std::ops::Add;

/// Time stored to be used as a specs resource.
#[derive(Default, PartialEq, PartialOrd, Clone, Copy)]
pub struct Time(pub u64);

impl Add for Time {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

/// SpriteIndexer is used to generate 'id' values on sprites.
/// it exists at runtime as a mutable specs resource.
#[derive(Default)]
pub struct SpriteIndexer(pub usize);
impl SpriteIndexer {
    /// gets a new index from SpriteIndexer, internally incrementing the counter. this reserves
    /// zero for 'none' or 'unknown' in later contexts.
    pub fn new_index(&mut self) -> usize {
        self.0 += 1;
        self.0
    }
}

/// Essential specs 'Position' component used with sprites.
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct Position {
    pub x: i64,
    pub y: i64,
    pub z: i64,
}

#[derive(Default, Debug, PartialOrd, PartialEq)]
pub enum SpriteType {
    #[default]
    Background,
    Overlay,
    Player,
    Tool,
    Crop,
    Particle,
}

/// Sprite is a specs component for sprites, and also tracks some game state.
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct Sprite {
    pub id: usize,
    pub store_index: usize,
    pub flip: bool,
    pub frame: usize,
    pub animating: bool,
    pub last_animate: u64,
    pub last_move: u64,
    pub sprite_type: SpriteType,
    pub highlight: bool,
    pub hidden: bool,
    pub delete: bool,
}

impl Default for Sprite {
    fn default() -> Self {
        Sprite {
            id: 0,
            store_index: 0,
            flip: false,
            frame: 0,
            animating: false,
            last_animate: 0,
            last_move: 0,
            sprite_type: SpriteType::Background,
            highlight: false,
            hidden: false,
            delete: false,
        }
    }
}

#[allow(dead_code)] // only necessary because Grass is not guaranteed to spawn
#[derive(Default, Debug, PartialOrd, PartialEq, Clone, Copy)]
pub enum ItemType {
    #[default]
    None,
    Grass,
    Pod,
    Terminal,
    Shovel,
    Watercan,
    Packet,
    Packet2,
    Crop,
    Npc,
}

pub struct SpriteDepths {
    pub ground: i64,
    pub grass: i64,
    pub crops: i64,
    pub player: i64,
    pub tools: i64,
    pub overlay: i64,
}

pub const DEPTHS: SpriteDepths = SpriteDepths {
    ground: 0,
    crops: 100_000,
    grass: 200_000,
    player: 300_000,
    tools: 400_000,
    overlay: 500_000,
};

#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct Npc {
    pub move_target: (i64, i64),
    pub last_move: u64,
    pub move_wait: u64,
    pub move_stop: u64,
}

/// specs component for interactive items.
#[derive(Component, Debug)]
#[storage(VecStorage)]
pub struct Interactible {
    pub item_type: ItemType,
    pub hold_to_use: bool,
}

/// specs resource used to store some global game state.
#[derive(Default)]
pub struct Game {
    pub holding: ItemType,
    pub show_help: bool,
    pub show_transition: bool,
    pub terminal_messages: Vec<String>,
    pub show_terminal: bool,
    pub terminal_message_index: usize,
    pub terminal_read: bool,
    pub clear_screen: bool,
}

impl Game {
    pub fn new(terminal_messages: Vec<String>) -> Game {
        Game {
            holding: ItemType::None,
            show_help: true,
            show_transition: true,
            terminal_messages,
            show_terminal: false,
            terminal_message_index: 0,
            terminal_read: false,
            clear_screen: false,
        }
    }

    pub fn advance_terminal(&mut self) {
        self.terminal_message_index += 1;
        self.terminal_read = false;
    }
}

fn nearest_of_type(
    from_pos: (i64, i64),
    search_type: SpriteType,
    store: &Read<'_, super::sprite::SpriteStore>,
    interactibles: &specs::Storage<
        '_,
        Interactible,
        specs::shred::Fetch<'_, specs::storage::MaskedStorage<Interactible>>,
    >,
    sprites: &mut specs::Storage<
        '_,
        Sprite,
        specs::shred::FetchMut<'_, specs::storage::MaskedStorage<Sprite>>,
    >,
    positions: &mut specs::Storage<
        '_,
        Position,
        specs::shred::FetchMut<'_, specs::storage::MaskedStorage<Position>>,
    >,
) -> (usize, i64, ItemType) {
    {
        use specs::Join;

        let mut tool_dist = 100;
        let mut tool_type = ItemType::None;
        let tool_id: usize = {
            let mut nearest_id = 0;
            for (item, sprite, pos) in (interactibles, sprites, positions).join() {
                if sprite.sprite_type != search_type {
                    continue;
                }
                let (sprite_w, sprite_h) = {
                    let s = &store.0[sprite.store_index];
                    (
                        s.data.frames[0].source_size.w as i64,
                        s.data.frames[0].source_size.h as i64,
                    )
                };
                let x = pos.x + sprite_w / 2;
                let y = pos.y + sprite_h / 4; // y coordinate space is 2 pixels per unit, so...
                let dist = (((x - from_pos.0) as f64).powi(2) + ((y - from_pos.1) as f64).powi(2))
                    .sqrt() as i64;
                if dist < tool_dist {
                    tool_dist = dist;
                    tool_type = item.item_type;
                    nearest_id = sprite.id;
                }
            }
            nearest_id
        };
        (tool_id, tool_dist, tool_type)
    }
}

/// big "update game state" specs system; for a simple game, it's ok... right?
pub struct UpdateGameState;
impl<'a> System<'a> for UpdateGameState {
    type SystemData = (
        Entities<'a>,
        Read<'a, LazyUpdate>,
        Write<'a, Game>,
        Write<'a, SpriteIndexer>,
        Read<'a, super::sprite::SpriteStore>,
        Read<'a, Time>,
        Read<'a, super::Input>,
        WriteStorage<'a, Sprite>,
        WriteStorage<'a, Position>,
        WriteStorage<'a, Npc>,
        ReadStorage<'a, Interactible>,
    );

    fn run(&mut self, data: Self::SystemData) {
        use super::InputState;
        use specs::Join;

        // initialize data
        const PICKUP_DISTANCE: i64 = 4;
        const CROP_DISTANCE: i64 = 2;
        let sz = terminal::size().unwrap();
        let (
            entities,
            lazy,
            mut game,
            mut si,
            store,
            time,
            input,
            mut sprites,
            mut positions,
            mut npcs,
            interactibles,
        ) = data;

        // get player position and flip values for use with items later
        let (player_pos, player_flip) = {
            let mut p_pos = (0, 0);
            let mut flip = false;
            for (sprite, pos) in (&sprites, &positions).join() {
                if sprite.sprite_type == SpriteType::Player {
                    p_pos = (pos.x, pos.y);
                    flip = sprite.flip;
                    break;
                }
            }
            (p_pos, flip)
        };
        let player_center = (player_pos.0 + 4, player_pos.1 + 2);

        // find nearest tool and its distance
        let (nearest_tool_id, nearest_tool_dist, nearest_tool_type) = nearest_of_type(
            player_center,
            SpriteType::Tool,
            &store,
            &interactibles,
            &mut sprites,
            &mut positions,
        );

        // offset from player from which to operate on crops
        let crop_pos = {
            let x = if player_flip { -9 } else { 0 };
            (player_center.0 + x, player_center.1)
        };

        // find nearest crop id (includes grass)
        let (nearest_crop_id, nearest_crop_dist, _) = nearest_of_type(
            (crop_pos.0 + 4, crop_pos.1 + 2),
            SpriteType::Crop,
            &store,
            &interactibles,
            &mut sprites,
            &mut positions,
        );

        for (item, sprite, pos) in (&interactibles, &mut sprites, &mut positions).join() {
            sprite.highlight = false;
            // control position of held items... this is extremely hacky; would be easier
            // to store an offset per interactible component. easy rewrite, but not vital.
            if game.holding == item.item_type {
                sprite.flip = player_flip;
                let wide_offset =
                    if item.item_type == ItemType::Pod || item.item_type == ItemType::Npc {
                        -3
                    } else {
                        0
                    };
                let action_offset = if input.0 == InputState::Action { 1 } else { 0 };
                let mut small_offset = 0;
                if game.holding == ItemType::Watercan || game.holding == ItemType::Terminal {
                    small_offset = 1;
                } else if game.holding == ItemType::Packet || game.holding == ItemType::Packet2 {
                    small_offset = 2;
                }

                pos.y = player_pos.1 + small_offset + action_offset + wide_offset;
                if pos.y < -2 {
                    pos.y = -2;
                }

                let flip_offset = if player_flip { -3 } else { 5 };
                pos.x = player_pos.0 + flip_offset + wide_offset;
                if pos.x < 0 {
                    pos.x = 0;
                }
                break;
            }

            // maintain or enable highlighting of nearby tool
            if game.holding == ItemType::None
                && nearest_tool_id == sprite.id
                && nearest_tool_dist < PICKUP_DISTANCE
            {
                sprite.highlight = true;
            }
        }

        // local types to act on a sprite after the following loop
        #[derive(PartialOrd, PartialEq)]
        enum SpriteActionCommand {
            Delete,
            Water,
            Seed,
            Seed2,
            Grow,
            None,
        }
        struct SpriteAction {
            id: usize,
            action: SpriteActionCommand,
        }
        let mut sprite_action: SpriteAction = SpriteAction {
            id: 0,
            action: SpriteActionCommand::None,
        };

        // make any NPC walk around randomly
        for (npc, pos, sprite) in (&mut npcs, &mut positions, &mut sprites).join() {
            if game.holding == ItemType::Npc {
                sprite.animating = false;
                sprite.frame = 1;
                continue;
            }
            let (target_x, target_y) = npc.move_target;
            if npc.last_move + npc.move_wait < time.0 {
                if pos.x != target_x {
                    sprite.animating = true;
                    if pos.x < target_x {
                        sprite.flip = false;
                        pos.x += 1;
                    } else {
                        sprite.flip = true;
                        pos.x -= 1;
                    }
                    npc.last_move = time.0;
                }
                if pos.y != target_y {
                    sprite.animating = true;
                    if pos.y < target_y {
                        pos.y += 1;
                    } else {
                        pos.y -= 1;
                    }
                    npc.last_move = time.0;
                }
                if pos.x == target_x && pos.y == target_y {
                    sprite.animating = false;
                    sprite.frame = 0;
                    if npc.last_move + npc.move_stop < time.0 {
                        // generate new move_target
                        use rand::Rng;
                        let mut rng = rand::thread_rng();
                        let (x, y) = (rng.gen_range(8..(sz.0 - 8)), rng.gen_range(4..(sz.1 - 4)));
                        npc.move_target = (x as i64, y as i64);
                    }
                }
            }
        }

        // sprites with positions get looped again to animate and handle the player
        for (sprite, pos) in (&mut sprites, &mut positions).join() {
            // reset overlay sprites if transition is requested
            if sprite.sprite_type == SpriteType::Overlay {
                let end_frame = store.0[sprite.store_index].data.frames.len() - 1;
                if game.show_transition {
                    sprite.animating = true;
                } else if sprite.frame == end_frame {
                    sprite.animating = false;
                }
            }

            // animate sprite frames by frame length in the loaded sprite metadata
            let sprite_data = &store.0[sprite.store_index];
            let frame_wait = sprite_data.data.frames[sprite.frame].duration as u64;
            if sprite.animating && sprite.last_animate + frame_wait < time.0 {
                sprite.last_animate = time.0;
                sprite.frame = (sprite.frame + 1) % sprite_data.data.frames.len();
            }

            // below here, only player sprite is handled
            if sprite.sprite_type != SpriteType::Player {
                continue;
            }

            // input parsing on player
            let mut rng = rand::thread_rng();
            let mut impulse = (0 as f64, 0 as f64);
            match input.0 {
                InputState::Left => {
                    impulse.0 = -2.0;
                }
                InputState::Up => {
                    impulse.1 = -1.0;
                }
                InputState::Down => {
                    impulse.1 = 1.0;
                }
                InputState::Right => {
                    impulse.0 = 2.0;
                }
                InputState::ShiftUp => {
                    impulse.1 = -2.0;
                }
                InputState::ShiftDown => {
                    impulse.1 = 2.0;
                }
                InputState::ShiftLeft => {
                    impulse.0 = -4.0;
                }
                InputState::ShiftRight => {
                    impulse.0 = 4.0;
                }
                InputState::Pickup => {
                    if game.holding == ItemType::None
                        && nearest_tool_type != ItemType::None
                        && nearest_tool_dist < PICKUP_DISTANCE
                    {
                        game.holding = nearest_tool_type;
                    } else if game.holding != ItemType::None {
                        game.holding = ItemType::None;
                    }
                }
                InputState::Action => 'action: {
                    if game.holding == ItemType::None && nearest_tool_type == ItemType::Pod {
                        sprite_action = SpriteAction {
                            id: 0,
                            action: SpriteActionCommand::Grow,
                        }
                    } else if game.holding == ItemType::None
                        && nearest_tool_type == ItemType::Terminal
                    {
                        if game.show_terminal {
                            game.show_terminal = false;
                            game.terminal_read = true;
                        } else if nearest_tool_dist <= PICKUP_DISTANCE {
                            game.show_terminal = true;
                        }
                    } else if game.holding == ItemType::Shovel {
                        // spawn a dirt particle
                        let e = entities.create();
                        let id = si.new_index();
                        lazy.insert(
                            e,
                            Sprite {
                                id,
                                store_index: store
                                    .index_by_name("particle-dirt")
                                    .expect("store index runtime error"),
                                sprite_type: SpriteType::Particle,
                                animating: true,
                                ..Sprite::default()
                            },
                        );
                        lazy.insert(
                            e,
                            Position {
                                x: crop_pos.0,
                                y: crop_pos.1,
                                z: DEPTHS.overlay + id as i64,
                            },
                        );

                        // find nearest grass or crop to dig up
                        if nearest_crop_dist < CROP_DISTANCE {
                            sprite_action = SpriteAction {
                                id: nearest_crop_id,
                                action: SpriteActionCommand::Delete,
                            };
                            break 'action;
                        }

                        // place new empty crop if none found
                        use rand::Rng;
                        let e = entities.create();
                        let id = si.new_index();
                        lazy.insert(
                            e,
                            Sprite {
                                id,
                                store_index: store
                                    .index_by_name("crop-empty")
                                    .expect("store index runtime error"),
                                flip: rng.gen_range(0..2) == 0,
                                sprite_type: SpriteType::Crop,
                                ..Sprite::default()
                            },
                        );
                        lazy.insert(
                            e,
                            Position {
                                x: crop_pos.0,
                                y: crop_pos.1,
                                z: DEPTHS.crops + id as i64,
                            },
                        );
                        lazy.insert(
                            e,
                            Interactible {
                                item_type: ItemType::Crop,
                                hold_to_use: false,
                            },
                        );
                    } else if game.holding == ItemType::Watercan {
                        // spawn a water particle
                        let e = entities.create();
                        let id = si.new_index();
                        lazy.insert(
                            e,
                            Sprite {
                                id,
                                store_index: store
                                    .index_by_name("particle-water")
                                    .expect("store index runtime error"),
                                sprite_type: SpriteType::Particle,
                                animating: true,
                                ..Sprite::default()
                            },
                        );
                        lazy.insert(
                            e,
                            Position {
                                x: crop_pos.0,
                                y: crop_pos.1,
                                z: DEPTHS.overlay + id as i64,
                            },
                        );
                        if nearest_crop_dist < CROP_DISTANCE {
                            sprite_action = SpriteAction {
                                id: nearest_crop_id,
                                action: SpriteActionCommand::Water,
                            };
                        }
                    } else if game.holding == ItemType::Packet {
                        if nearest_crop_dist < CROP_DISTANCE {
                            sprite_action = SpriteAction {
                                id: nearest_crop_id,
                                action: SpriteActionCommand::Seed,
                            };
                        }
                    } else if game.holding == ItemType::Packet2 {
                        if nearest_crop_dist < CROP_DISTANCE {
                            sprite_action = SpriteAction {
                                id: nearest_crop_id,
                                action: SpriteActionCommand::Seed2,
                            };
                        }
                    } else if ((game.holding == ItemType::None) || (game.holding == ItemType::Npc))
                        && nearest_tool_type == ItemType::Npc
                    {
                        // spawn a heart on the player!
                        let e = entities.create();
                        let id = si.new_index();
                        lazy.insert(
                            e,
                            Sprite {
                                id,
                                store_index: store
                                    .index_by_name("particle-heart")
                                    .expect("store index runtime error"),
                                sprite_type: SpriteType::Particle,
                                animating: true,
                                ..Sprite::default()
                            },
                        );
                        lazy.insert(
                            e,
                            Position {
                                x: player_center.0,
                                y: player_center.1 - 2,
                                z: DEPTHS.overlay + id as i64,
                            },
                        );
                    }
                }
                InputState::ToggleHelp => {
                    game.show_help = !game.show_help;
                }
                InputState::Quit => {}
                InputState::Clear => {
                    game.clear_screen = true;
                }
                InputState::None => {
                    // when the player sprite stops moving, it stops animating after this many milliseconds.
                    if sprite.last_move + 400 < time.0 {
                        sprite.animating = false;
                        sprite.frame = 0;
                    }
                    game.clear_screen = false;
                }
            }

            // player movement
            if impulse.0.abs() > 0.0 || impulse.1.abs() > 0.0 {
                if game.show_terminal {
                    game.show_terminal = false;
                }

                if impulse.0 < 0.0 {
                    sprite.flip = true;
                } else if impulse.0 > 0.0 {
                    sprite.flip = false;
                }
                sprite.animating = true;
                if sprite.last_move + frame_wait / 2 < time.0 {
                    sprite.last_move = time.0;
                    pos.x += impulse.0 as i64;
                    pos.y += impulse.1 as i64;

                    // clamp pos.x to 0, sz.0
                    if pos.x < 0 {
                        pos.x = 0;
                    } else if pos.x > sz.0 as i64 - 10 {
                        pos.x = sz.0 as i64 - 10;
                    }

                    // clamp pos.y to -2, sz.1
                    if pos.y < -2 {
                        pos.y = -2;
                    } else if pos.y > sz.1 as i64 - 5 {
                        pos.y = sz.1 as i64 - 5;
                    }
                }
            }
        }

        if game.show_transition {
            game.show_transition = false;
        }

        // if the command is Grow, the player slept, so animate the transition effect
        // and check game state for terminal story sequence progression
        if sprite_action.action == SpriteActionCommand::Grow {
            game.show_transition = true;
            for (sprite, _pos) in (&mut sprites, &positions).join() {
                if sprite.sprite_type == SpriteType::Overlay {
                    sprite.frame = 0;
                }
            }
            match game.terminal_message_index {
                0 => {
                    // introductory message progresses story once read
                    if game.terminal_read {
                        game.advance_terminal();
                    }
                }
                1 => {
                    // "water your crops" message progresses story once a crop has been watered
                    for (item, sprite) in (&interactibles, &mut sprites).join() {
                        if item.item_type == ItemType::Crop && sprite.frame > 0 {
                            game.advance_terminal();
                            break;
                        }
                    }
                }
                2 => {
                    // K intro message progresses once read, and spawns Packet2
                    if game.terminal_read {
                        game.advance_terminal();
                        let e = entities.create();
                        let id = si.new_index();
                        lazy.insert(
                            e,
                            Sprite {
                                id,
                                store_index: store
                                    .index_by_name("tool-packet2")
                                    .expect("store index runtime error"),
                                sprite_type: SpriteType::Tool,
                                ..Sprite::default()
                            },
                        );
                        lazy.insert(
                            e,
                            Position {
                                x: sz.0 as i64 / 2,
                                y: sz.1 as i64 / 2,
                                z: DEPTHS.tools + id as i64,
                            },
                        );
                        lazy.insert(
                            e,
                            Interactible {
                                item_type: ItemType::Packet2,
                                hold_to_use: true,
                            },
                        );
                    }
                }
                3 => {
                    // if the player seeds with Packet2, the next message gets displayed
                    for (item, sprite) in (&interactibles, &mut sprites).join() {
                        if item.item_type == ItemType::Crop
                            && sprite.store_index
                                == store.index_by_name("crop-flower").expect("store error")
                            && game.terminal_read
                        {
                            game.advance_terminal();
                            break;
                        }
                    }
                }
                4 => {
                    // if the player progresses Packet2 seeds past 2 frames, next message
                    for (item, sprite) in (&interactibles, &mut sprites).join() {
                        if item.item_type == ItemType::Crop
                            && sprite.store_index
                                == store.index_by_name("crop-flower").expect("store error")
                            && game.terminal_read
                            && ((sprite.frame == 2) || (sprite.frame == 5))
                        {
                            game.advance_terminal();
                            break;
                        }
                    }
                }
                5 => {
                    // progresses once read
                    if game.terminal_read {
                        game.advance_terminal();
                    }
                }
                6 => {
                    // progresses once a flower blooms
                    for (item, sprite) in (&interactibles, &mut sprites).join() {
                        if item.item_type == ItemType::Crop
                            && sprite.store_index
                                == store.index_by_name("crop-flower").expect("store error")
                            && game.terminal_read
                            && ((sprite.frame == 3) || (sprite.frame == 6))
                        {
                            game.advance_terminal();
                            break;
                        }
                    }
                }
                7 => {
                    // progresses once read, and spawns NPC
                    if game.terminal_read {
                        game.advance_terminal();

                        let e = entities.create();
                        let id = si.new_index();
                        lazy.insert(
                            e,
                            Sprite {
                                id,
                                store_index: store
                                    .index_by_name("character-01")
                                    .expect("store index runtime error"),
                                sprite_type: SpriteType::Tool,
                                ..Sprite::default()
                            },
                        );
                        lazy.insert(
                            e,
                            Position {
                                x: sz.0 as i64 / 2,
                                y: 0,
                                z: DEPTHS.player + id as i64,
                            },
                        );
                        lazy.insert(
                            e,
                            Interactible {
                                item_type: ItemType::Npc,
                                hold_to_use: false,
                            },
                        );
                        lazy.insert(
                            e,
                            Npc {
                                move_target: (sz.0 as i64 / 2, sz.1 as i64 / 2),
                                last_move: time.0,
                                move_wait: 200,
                                move_stop: 2000,
                            },
                        );
                    }
                }
                8 => {
                    // progresses once read
                    if game.terminal_read {
                        game.advance_terminal();
                    }
                }
                _ => {}
            }
        }

        // remove particles that reach the end of their animation
        for (entity, sprite) in (&entities, &sprites).join() {
            if sprite.sprite_type == SpriteType::Particle {
                let end_frame = store.0[sprite.store_index].data.frames.len() - 1;
                if sprite.frame == end_frame {
                    lazy.remove::<Sprite>(entity);
                    lazy.remove::<Position>(entity);
                }
            }
        }

        // perform actions - grow, water, seed, or tag crop entities for deletion
        for (entity, item, sprite, pos) in
            (&entities, &interactibles, &mut sprites, &positions).join()
        {
            // if terminal is read, stop animating
            if item.item_type == ItemType::Terminal {
                if game.terminal_read {
                    sprite.frame = 0;
                    sprite.animating = false;
                } else {
                    sprite.animating = true;
                }
                continue;
            }

            // grow all crops that were watered
            if sprite_action.action == SpriteActionCommand::Grow && item.item_type == ItemType::Crop
            {
                if sprite.frame < 4 {
                    continue;
                } else if sprite.frame < 7 {
                    sprite.frame = sprite.frame - 4 + 1;
                } else if sprite.frame == 7 {
                    // in the case of empty crops that are watered or grow to frame 7,
                    // remove this crop and replace it with animated grass
                    lazy.remove::<Sprite>(entity);
                    lazy.remove::<Position>(entity);
                    lazy.remove::<Interactible>(entity);

                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    let e = entities.create();
                    let id = si.new_index();
                    let grass_frame_count = store.0[sprite.store_index].data.frames.len();
                    lazy.insert(
                        e,
                        Sprite {
                            id,
                            store_index: store
                                .index_by_name("grass")
                                .expect("store index runtime error"),
                            flip: rng.gen_range(0..2) == 0,
                            frame: rng.gen_range(0..grass_frame_count),
                            animating: true,
                            sprite_type: SpriteType::Crop,
                            ..Sprite::default()
                        },
                    );
                    lazy.insert(
                        e,
                        Position {
                            x: pos.x,
                            y: pos.y,
                            z: DEPTHS.crops + id as i64,
                        },
                    );
                    lazy.insert(
                        e,
                        Interactible {
                            item_type: ItemType::Grass,
                            hold_to_use: false,
                        },
                    );
                }
            }

            // below actions only operate on single sprites
            if sprite_action.id != sprite.id {
                continue;
            }

            if sprite_action.action == SpriteActionCommand::Water && sprite.frame < 4 {
                sprite.frame += 4;
            } else if sprite_action.action == SpriteActionCommand::Seed {
                if sprite.store_index
                    == store
                        .index_by_name("crop-empty")
                        .expect("store index error")
                {
                    sprite.frame = 0;
                    sprite.store_index =
                        store.index_by_name("crop-leaf").expect("store index error");
                }
            } else if sprite_action.action == SpriteActionCommand::Seed2 {
                if sprite.store_index
                    == store
                        .index_by_name("crop-empty")
                        .expect("store index error")
                {
                    sprite.frame = 0;
                    sprite.store_index = store
                        .index_by_name("crop-flower")
                        .expect("store index error");
                }
            } else if sprite_action.action == SpriteActionCommand::Delete {
                lazy.remove::<Sprite>(entity);
                lazy.remove::<Position>(entity);
                lazy.remove::<Interactible>(entity);
            }
        }
    }
}
