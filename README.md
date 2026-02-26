# Roguelike MVP

A deterministic, ASCII-grid roguelike built in Rust using Macroquad.

## Architecture
The project is split into three main crates to enforce separation of concerns and deterministic logic:
- `core`: The pure, headless deterministic simulation engine, including hardcoded item/content definitions.
- `app`: The Macroquad frontend handling all UI, input translation, and rendering functions.
- `tools`: A suite of headless tools for testing replays and balancing the game computationally without UI overhead.

## Running the App
To start the game visually:
```bash
cargo run --bin app
```

## Running headless tests/tools
```bash
cargo run --bin tools
cargo test --workspace
```
