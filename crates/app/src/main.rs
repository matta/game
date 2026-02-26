use macroquad::prelude::*;

#[macroquad::main("Roguelike")]
async fn main() {
    loop {
        clear_background(BLACK);
        draw_text("MVP Scaffolding Installed", 20.0, 20.0, 30.0, DARKGRAY);
        next_frame().await
    }
}
