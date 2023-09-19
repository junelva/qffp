# queer folk farmpunk: farming on the moon

![screenshot](res/screenshot.png)

#### Status

Works well on WSL, assume it works on Linux; input is busted on Powershell but the renderer does work.

### What is it?

"Queer Folk Farmpunk" is a larger game concept I've been toying with for a while. I decided to create a demake of sorts, heavily inspired by Stardew Valley, to familiarize myself with the creation of custom miniature game engines.

`qffp` is a technical game prototype demonstrating the possibility of creating low-res games with animated pixel art that run in a terminal. This implementation is written in Rust and makes primary use of three crates: [crossterm](https://github.com/crossterm-rs/crossterm) for terminal input and low-level windowing, [specs](https://github.com/amethyst/specs) for a versatile entity component system, and [anathema::display](https://github.com/togglebyte/anathema) for flicker-free, double-buffered, immediate-mode rendering.

One interesting quality of a pixel buffer being used in a terminal is that the text is inherently higher resolution than the graphics, creating cozy yet readable output.

Developed in a Linux terminal, the pixel art was drawn in Aseprite.

#### Running

You will need the Rust compiler and package manager, `cargo`. If you run the crate with `cargo run`, debug information will be displayed. To run without debug information, use `cargo run -r` to build and run the release version. It's likely that this command must be run while in the base project directory (`qffp`).

Your terminal font must support unicode half-block characters and your terminal must be in 256 color mode.

### Analysis

#### Lessons

qffp is my first finished game prototype in the Rust language. I learned a lot about the language while using it and did not encounter anything too difficult. Alongside the Rust experience, I became more familiar with specs, the ECS crate I used. The strength of its design became apparent when rendering sorted, animated text sprites in an earlier prototype.

#### Things I would do differently

I implemented sprite rendering early on and did not create an intermediary buffer upon which to render pixels before copying to the terminal using Unicode half-block characters. This means when rendering transparent pixels I need to perform logic on the existing text buffer, which is limited by the fact that the Y axis counts by 2 - because each character cell is two pixels stacked vertically.

The renderer would be more flexible if everything was rendered to an internal pixel buffer first. It technically is double buffered with the display module, but that's another text buffer, not a pixel buffer. All this results in game logic needing to count the y axis of sprites by 2, which is not ideal.

### Todo

- [x] custom animated pixel art
- [x] image sprite loading and rendering
- [x] input state passable to specs systems
- [x] "depth" sorting of sprites
- [x] animated particle sprites
- [x] entities have unique ids
- [x] dig with shovel: grass or crop near, uproot. nothing near, place empty crop
- [x] seed packet: empty crop near, change empty crop to leaf crop
- [x] watercan: crop near, make watered; watered crops grow when sleeping
- [x] pod: use to sleep
- [x] transition animation on sleep
- [x] terminal: displays sequence of messages to progress story between sleep cycles
- [x] make it gay

