//! Executor with your game connected to it as a plugin.
use fyrox::engine::executor::Executor;
use fyrox::engine::GraphicsContextParams;
use fyrox::event_loop::EventLoop;

fn main() {
    let mut executor = Executor::from_params(
        Some(EventLoop::new().unwrap()),
        GraphicsContextParams {
            window_attributes: Default::default(),
            vsync: false,
            msaa_sample_count: None,
            graphics_server_constructor: Default::default(),
            named_objects: false,
        },
    );

    executor.set_throttle_frame_interval(1000);

    // Dynamic linking with hot reloading.
    #[cfg(feature = "dylib")]
    {
        #[cfg(target_os = "windows")]
        let file_name = "game_dylib.dll";
        #[cfg(target_os = "linux")]
        let file_name = "libgame_dylib.so";
        #[cfg(target_os = "macos")]
        let file_name = "libgame_dylib.dylib";
        executor.add_dynamic_plugin(file_name, true, true).unwrap();
    }

    // Static linking.
    #[cfg(not(feature = "dylib"))]
    {
        use station_iapetus::Game;
        executor.add_plugin(Game::default());
    }

    executor.run()
}
