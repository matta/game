# Roguelike MVP

A deterministic, ASCII-grid roguelike built in Rust using Macroquad.

## Post Mortem

Turns out I'm not all that interested in developing games! I started this
when I felt ill, but when energy returned my interest in this waned. I
probably should have just played DCSS.

I was also more interested in the architectural decisions, and a whole lot
less interested in the actual game-making. The creative part of making a
game is never been a strong pull.

This has been a good reminder.

This was a nice, focused, way to learn a bit about AI driven software
engineering techniques.

## Architecture
The project is split into two main crates to enforce separation of concerns and deterministic logic:
- `core`: The pure, headless deterministic simulation engine, including hardcoded item/content definitions.
- `app`: The Macroquad frontend handling all UI, input translation, and rendering functions.

## Running the App
To start the game visually:
```bash
cargo run --bin app
```

## Running headless tests
```bash
cargo test --workspace
cargo test -p core --test semantic_fuzz --release
```
