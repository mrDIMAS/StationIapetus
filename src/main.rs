extern crate rg3d;

pub mod actor;
pub mod bot;
pub mod character;
pub mod control_scheme;
pub mod effects;
pub mod gui;
pub mod hud;
pub mod item;
pub mod level;
pub mod menu;
pub mod message;
pub mod options_menu;
pub mod player;
pub mod projectile;
pub mod weapon;

use crate::{
    actor::Actor, control_scheme::ControlScheme, hud::Hud, level::Level, menu::Menu,
    message::Message,
};
use rg3d::{
    core::{
        color::Color,
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::Engine,
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::{
        grid::{Column, GridBuilder, Row},
        message::ProgressBarMessage,
        message::{MessageDirection, TextMessage, UiMessage, WidgetMessage},
        node::{StubNode, UINode},
        progress_bar::ProgressBarBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        HorizontalAlignment, UserInterface, VerticalAlignment,
    },
    scene::Scene,
    sound::{
        context::Context,
        source::{generic::GenericSourceBuilder, SoundSource, Status},
    },
    utils::{
        log::{Log, MessageKind},
        translate_event,
    },
};
use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{self, Duration, Instant},
};

const FIXED_FPS: f32 = 60.0;

// Define type aliases for engine structs.
pub type UiNode = UINode<(), StubNode>;
pub type UINodeHandle = Handle<UiNode>;
pub type GameEngine = Engine<(), StubNode>;
pub type Gui = UserInterface<(), StubNode>;
pub type GuiMessage = UiMessage<(), StubNode>;
pub type BuildContext<'a> = rg3d::gui::BuildContext<'a, (), StubNode>;

pub struct Game {
    menu: Menu,
    hud: Hud,
    engine: GameEngine,
    level: Option<Level>,
    debug_text: UINodeHandle,
    debug_string: String,
    last_tick_time: time::Instant,
    running: bool,
    control_scheme: Arc<RwLock<ControlScheme>>,
    time: GameTime,
    events_receiver: Receiver<Message>,
    events_sender: Sender<Message>,
    load_context: Option<Arc<Mutex<LoadContext>>>,
    loading_screen: LoadingScreen,
    menu_sound_context: Context,
    music: Handle<SoundSource>,
}

struct LoadingScreen {
    root: Handle<UiNode>,
    progress_bar: Handle<UiNode>,
}

impl LoadingScreen {
    fn new(ctx: &mut BuildContext, width: f32, height: f32) -> Self {
        let progress_bar;
        let root = GridBuilder::new(
            WidgetBuilder::new()
                .with_width(width)
                .with_height(height)
                .with_visibility(false)
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .on_row(1)
                            .on_column(1)
                            .with_child({
                                progress_bar =
                                    ProgressBarBuilder::new(WidgetBuilder::new().on_row(1))
                                        .build(ctx);
                                progress_bar
                            })
                            .with_child(
                                TextBuilder::new(WidgetBuilder::new().on_row(0))
                                    .with_horizontal_text_alignment(HorizontalAlignment::Center)
                                    .with_vertical_text_alignment(VerticalAlignment::Center)
                                    .with_text("Loading... Please wait.")
                                    .build(ctx),
                            ),
                    )
                    .add_row(Row::stretch())
                    .add_row(Row::strict(32.0))
                    .add_column(Column::stretch())
                    .build(ctx),
                ),
        )
        .add_column(Column::stretch())
        .add_column(Column::strict(400.0))
        .add_column(Column::stretch())
        .add_row(Row::stretch())
        .add_row(Row::strict(100.0))
        .add_row(Row::stretch())
        .build(ctx);
        Self { root, progress_bar }
    }
}

#[derive(Copy, Clone)]
pub struct GameTime {
    clock: time::Instant,
    elapsed: f64,
    delta: f32,
}

// Disable false-positive lint, isize *is* portable.
#[allow(clippy::enum_clike_unportable_variant)]
pub enum CollisionGroups {
    Generic = 1,
    Projectile = 1 << 1,
    Actor = 1 << 2,
    All = std::isize::MAX,
}

pub struct LoadContext {
    level: Option<(Level, Scene)>,
}

impl Game {
    pub fn run() {
        let events_loop = EventLoop::<()>::new();

        let primary_monitor = events_loop.primary_monitor().unwrap();
        let mut monitor_dimensions = primary_monitor.size();
        monitor_dimensions.height = (monitor_dimensions.height as f32 * 0.7) as u32;
        monitor_dimensions.width = (monitor_dimensions.width as f32 * 0.7) as u32;
        let inner_size = monitor_dimensions.to_logical::<f32>(primary_monitor.scale_factor());

        let window_builder = rg3d::window::WindowBuilder::new()
            .with_title("Rusty Shooter")
            .with_inner_size(inner_size)
            .with_resizable(true);

        let mut engine = GameEngine::new(window_builder, &events_loop, false).unwrap();

        engine.renderer.set_ambient_color(Color::opaque(60, 60, 60));

        let control_scheme = Arc::new(RwLock::new(ControlScheme::default()));

        let fixed_timestep = 1.0 / FIXED_FPS;

        let time = GameTime {
            clock: Instant::now(),
            elapsed: 0.0,
            delta: fixed_timestep,
        };

        let (tx, rx) = mpsc::channel();

        let menu_sound_context = Context::new();

        let buffer = rg3d::futures::executor::block_on(
            engine
                .resource_manager
                .request_sound_buffer("data/sounds/Antonio_Bizarro_Berzerker.ogg", true),
        )
        .unwrap();
        let music = menu_sound_context.state().add_source(
            GenericSourceBuilder::new(buffer.into())
                .with_looping(true)
                .with_status(Status::Playing)
                .with_gain(0.0)
                .build_source()
                .unwrap(),
        );

        engine
            .sound_engine
            .lock()
            .unwrap()
            .add_context(menu_sound_context.clone());

        let mut game = Game {
            loading_screen: LoadingScreen::new(
                &mut engine.user_interface.build_ctx(),
                inner_size.width,
                inner_size.height,
            ),
            menu_sound_context,
            music,
            hud: Hud::new(&mut engine),
            running: true,
            menu: Menu::new(&mut engine, control_scheme.clone(), tx.clone()),
            control_scheme,
            debug_text: Handle::NONE,
            engine,
            level: None,
            debug_string: String::new(),
            last_tick_time: time::Instant::now(),
            time,
            events_receiver: rx,
            events_sender: tx,
            load_context: None,
        };

        game.create_debug_ui();

        events_loop.run(move |event, _, control_flow| {
            game.process_input_event(&event);

            match event {
                Event::MainEventsCleared => {
                    let mut dt = game.time.clock.elapsed().as_secs_f64() - game.time.elapsed;
                    while dt >= fixed_timestep as f64 {
                        dt -= fixed_timestep as f64;
                        game.time.elapsed += fixed_timestep as f64;

                        game.update(game.time);

                        while let Some(ui_event) = game.engine.user_interface.poll_message() {
                            game.menu.handle_ui_event(
                                &mut game.engine,
                                game.level.as_ref(),
                                &ui_event,
                            );
                        }
                    }
                    if !game.running {
                        *control_flow = ControlFlow::Exit;
                    }
                    game.engine.get_window().request_redraw();
                }
                Event::RedrawRequested(_) => {
                    game.update_statistics(game.time.elapsed);

                    // <<<<< ENABLE THIS TO SHOW DEBUG GEOMETRY >>>>>
                    if false {
                        game.debug_render();
                    }

                    // Render at max speed
                    game.engine.render(fixed_timestep).unwrap();
                    // Make sure to cap update rate to 60 FPS.
                    game.limit_fps(FIXED_FPS as f64);
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        game.destroy_level();
                        *control_flow = ControlFlow::Exit
                    }
                    WindowEvent::Resized(new_size) => {
                        game.engine.renderer.set_frame_size(new_size.into());
                    }
                    _ => (),
                },
                Event::LoopDestroyed => {
                    rg3d::core::profiler::print();
                }
                _ => *control_flow = ControlFlow::Poll,
            }
        });
    }

    fn debug_render(&mut self) {
        if let Some(level) = self.level.as_mut() {
            level.debug_draw(&mut self.engine);
        }
    }

    pub fn create_debug_ui(&mut self) {
        self.debug_text = TextBuilder::new(WidgetBuilder::new().with_width(400.0))
            .build(&mut self.engine.user_interface.build_ctx());
    }

    pub fn save_game(&mut self) -> VisitResult {
        let mut visitor = Visitor::new();

        // Visit engine state first.
        self.engine.visit("GameEngine", &mut visitor)?;
        self.level.visit("Level", &mut visitor)?;
        self.menu_sound_context
            .visit("MenuSoundContext", &mut visitor)?;
        self.music.visit("Music", &mut visitor)?;

        // Debug output
        if let Ok(mut file) = File::create(Path::new("save.txt")) {
            file.write_all(visitor.save_text().as_bytes()).unwrap();
        }

        visitor.save_binary(Path::new("save.bin"))
    }

    pub fn load_game(&mut self) -> VisitResult {
        Log::writeln(
            MessageKind::Information,
            "Attempting load a save...".to_owned(),
        );

        let mut visitor = Visitor::load_binary(Path::new("save.bin"))?;

        // Clean up.
        self.destroy_level();

        // Load engine state first
        Log::writeln(
            MessageKind::Information,
            "Trying to load a save file...".to_owned(),
        );
        self.engine.visit("GameEngine", &mut visitor)?;
        self.level.visit("Level", &mut visitor)?;
        self.menu_sound_context
            .visit("MenuSoundContext", &mut visitor)?;
        self.music.visit("Music", &mut visitor)?;

        Log::writeln(
            MessageKind::Information,
            "Game state successfully loaded!".to_owned(),
        );

        // Hide menu only of we successfully loaded a save.
        self.set_menu_visible(false);

        // Set control scheme for player.
        if let Some(level) = &mut self.level {
            level.set_message_sender(self.events_sender.clone(), &mut self.engine);
            level.control_scheme = Some(self.control_scheme.clone());
            let player = level.get_player();
            if let Actor::Player(player) = level.actors_mut().get_mut(player) {
                player.set_control_scheme(self.control_scheme.clone());
            }
        }

        self.time.elapsed = self.time.clock.elapsed().as_secs_f64();

        Ok(())
    }

    fn destroy_level(&mut self) {
        if let Some(ref mut level) = self.level.take() {
            level.destroy(&mut self.engine);
            Log::writeln(
                MessageKind::Information,
                "Current level destroyed!".to_owned(),
            );
        }
    }

    pub fn start_new_game(&mut self) {
        self.destroy_level();

        let ctx = Arc::new(Mutex::new(LoadContext { level: None }));

        self.load_context = Some(ctx.clone());

        self.engine
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.loading_screen.root,
                MessageDirection::ToWidget,
                true,
            ));
        self.menu
            .set_visible(&mut self.engine.user_interface, false);

        let resource_manager = self.engine.resource_manager.clone();
        let control_scheme = self.control_scheme.clone();
        let sender = self.events_sender.clone();

        std::thread::spawn(move || {
            let level = rg3d::futures::executor::block_on(Level::new(
                resource_manager,
                control_scheme,
                sender,
            ));

            ctx.lock().unwrap().level = Some(level);
        });
    }

    pub fn set_menu_visible(&mut self, visible: bool) {
        let ui = &mut self.engine.user_interface;
        self.menu.set_visible(ui, visible);
        self.hud.set_visible(ui, !visible);
    }

    pub fn is_menu_visible(&self) -> bool {
        self.menu.is_visible(&self.engine.user_interface)
    }

    pub fn update(&mut self, time: GameTime) {
        let window = self.engine.get_window();
        window.set_cursor_visible(self.is_menu_visible());
        let _ = window.set_cursor_grab(!self.is_menu_visible());

        if let Some(ctx) = self.load_context.clone() {
            if let Ok(mut ctx) = ctx.try_lock() {
                if let Some((mut level, scene)) = ctx.level.take() {
                    level.scene = self.engine.scenes.add(scene);
                    self.level = Some(level);
                    self.load_context = None;
                    self.set_menu_visible(false);
                    self.engine
                        .user_interface
                        .send_message(WidgetMessage::visibility(
                            self.loading_screen.root,
                            MessageDirection::ToWidget,
                            false,
                        ));
                } else {
                    self.engine
                        .user_interface
                        .send_message(ProgressBarMessage::progress(
                            self.loading_screen.progress_bar,
                            MessageDirection::ToWidget,
                            self.engine.resource_manager.state().loading_progress() as f32 / 100.0,
                        ));
                }
            }
        }

        self.engine.update(time.delta);

        if let Some(ref mut level) = self.level {
            level.update(&mut self.engine, time);
            let ui = &mut self.engine.user_interface;
            let player = level.get_player();
            if player.is_some() {
                // Sync hud with player state.
                let player = level.actors().get(player);
                self.hud.set_health(ui, player.get_health());
                self.hud.set_armor(ui, player.get_armor());
                let current_weapon = player.current_weapon();
                if current_weapon.is_some() {
                    self.hud
                        .set_ammo(ui, level.weapons()[current_weapon].ammo());
                }
                self.hud.set_is_died(ui, false);
            } else {
                self.hud.set_is_died(ui, true);
            }
        }

        self.handle_messages(time);
    }

    fn handle_messages(&mut self, time: GameTime) {
        while let Ok(message) = self.events_receiver.try_recv() {
            match &message {
                Message::StartNewGame => {
                    self.start_new_game();
                }
                Message::SaveGame => match self.save_game() {
                    Ok(_) => {
                        Log::writeln(MessageKind::Information, "Successfully saved".to_owned())
                    }
                    Err(e) => Log::writeln(
                        MessageKind::Error,
                        format!("Failed to make a save, reason: {}", e),
                    ),
                },
                Message::LoadGame => {
                    if let Err(e) = self.load_game() {
                        Log::writeln(
                            MessageKind::Error,
                            format!("Failed to load saved game. Reason: {:?}", e),
                        );
                    }
                }
                Message::QuitGame => {
                    self.destroy_level();
                    self.running = false;
                }
                Message::EndMatch => {
                    self.destroy_level();
                }
                Message::SetMusicVolume { volume } => {
                    self.menu_sound_context
                        .state()
                        .source_mut(self.music)
                        .set_gain(*volume);
                }
                _ => (),
            }

            if let Some(ref mut level) = self.level {
                rg3d::futures::executor::block_on(level.handle_message(
                    &mut self.engine,
                    &message,
                    time,
                ));
            }
        }
    }

    pub fn update_statistics(&mut self, elapsed: f64) {
        self.debug_string.clear();
        use std::fmt::Write;
        let statistics = self.engine.renderer.get_statistics();
        write!(
            self.debug_string,
            "Pure frame time: {:.2} ms\n\
               Capped frame time: {:.2} ms\n\
               FPS: {}\n\
               Triangles: {}\n\
               Draw calls: {}\n\
               Uptime: {:.2} s\n\
               UI time: {:?}",
            statistics.pure_frame_time * 1000.0,
            statistics.capped_frame_time * 1000.0,
            statistics.frames_per_second,
            statistics.geometry.triangles_rendered,
            statistics.geometry.draw_calls,
            elapsed,
            self.engine.ui_time
        )
        .unwrap();

        self.engine.user_interface.send_message(TextMessage::text(
            self.debug_text,
            MessageDirection::ToWidget,
            self.debug_string.clone(),
        ));
    }

    pub fn limit_fps(&mut self, value: f64) {
        let current_time = time::Instant::now();
        let render_call_duration = current_time
            .duration_since(self.last_tick_time)
            .as_secs_f64();
        self.last_tick_time = current_time;
        let desired_frame_time = 1.0 / value;
        if render_call_duration < desired_frame_time {
            thread::sleep(Duration::from_secs_f64(
                desired_frame_time - render_call_duration,
            ));
        }
    }

    fn process_dispatched_event(&mut self, event: &Event<()>) {
        if let Event::WindowEvent { event, .. } = event {
            if let Some(event) = translate_event(event) {
                self.engine.user_interface.process_os_event(&event);
            }
        }

        if !self.is_menu_visible() {
            if let Some(ref mut level) = self.level {
                level.process_input_event(
                    event,
                    &mut self.engine.scenes[level.scene],
                    self.time.delta,
                );
            }
        }
    }

    pub fn process_input_event(&mut self, event: &Event<()>) {
        self.process_dispatched_event(event);

        if let Event::WindowEvent { event, .. } = event {
            if let WindowEvent::KeyboardInput { input, .. } = event {
                if let ElementState::Pressed = input.state {
                    if let Some(key) = input.virtual_keycode {
                        if key == VirtualKeyCode::Escape && self.level.is_some() {
                            self.set_menu_visible(!self.is_menu_visible());
                        }
                    }
                }
            }
        }

        self.menu.process_input_event(&mut self.engine, &event);
        self.hud.process_event(&mut self.engine, &event);
    }
}

fn main() {
    Game::run();
}
