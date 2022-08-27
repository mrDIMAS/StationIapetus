//! Editor with your game connected to it as a plugin.
use fyrox::event_loop::EventLoop;
use fyroxed_base::{Editor, StartupData};
use station_iapetus::GameConstructor;

fn main() {
    let event_loop = EventLoop::new();
    let mut editor = Editor::new(
        &event_loop,
        Some(StartupData {
            working_directory: Default::default(),
            scene: "data/levels/loading_bay.rgs".into(),
        }),
    );
    editor.add_game_plugin(GameConstructor);
    editor.run(event_loop)
}
