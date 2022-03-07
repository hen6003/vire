mod editor;
mod text_buf;

use editor::Editor;

fn main() {
    let file = std::env::args().into_iter().nth(1);

    // Start editor with provided file
    Editor::new(file)
        .default_environment()
        .start_loop();
}
