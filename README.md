# Roguelike MVP

A deterministic, ASCII-grid roguelike built in Rust using Macroquad.

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
