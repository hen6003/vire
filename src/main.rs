mod editor;
mod text_buf;

fn main() {
    editor::Editor::new()
        .default_environment()
        .start_loop();
}
