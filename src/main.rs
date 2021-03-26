#![allow(clippy::too_many_arguments)]

extern crate rg3d;
extern crate ron;
extern crate serde;

pub mod actor;
pub mod bot;
pub mod character;
pub mod config;
pub mod control_scheme;
pub mod door;
pub mod effects;
pub mod gui;
pub mod inventory;
pub mod item;
pub mod level;
pub mod light;
pub mod menu;
pub mod message;
pub mod options_menu;
pub mod player;
pub mod sound;
pub mod utils;
pub mod weapon;

use crate::{
    actor::Actor,
    config::Config,
    control_scheme::ControlScheme,
    gui::{
        inventory::InventoryInterface, item_display::ItemDisplay, weapon_display::WeaponDisplay,
        BuildContext, CustomUiMessage, CustomUiNode, DeathScreen, GuiMessage, UiNode, UiNodeHandle,
    },
    level::Level,
    menu::Menu,
    message::Message,
};
use rg3d::{
    animation::{
        machine::{Machine, PoseNode, State},
        Animation,
    },
    core::{
        algebra::{UnitQuaternion, Vector3},
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    dpi::LogicalSize,
    engine::{resource_manager::ResourceManager, Engine},
    event::{ElementState, Event, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    gui::{
        grid::{Column, GridBuilder, Row},
        message::{
            ButtonMessage, CheckBoxMessage, MessageDirection, ProgressBarMessage, TextMessage,
            UiMessageData, WidgetMessage,
        },
        progress_bar::ProgressBarBuilder,
        text::TextBuilder,
        ttf::{Font, SharedFont},
        widget::WidgetBuilder,
        HorizontalAlignment, VerticalAlignment,
    },
    resource::model::Model,
    scene::{node::Node, Scene},
    sound::source::{generic::GenericSourceBuilder, Status},
    utils::{
        log::{Log, MessageKind},
        translate_event,
    },
};
use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    ops::Index,
    path::{Path, PathBuf},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
    time::{self, Duration, Instant},
};

const FIXED_FPS: f32 = 60.0;

// Define type aliases for engine structs.
pub type GameEngine = Engine<CustomUiMessage, CustomUiNode>;

pub fn create_play_animation_state(
    animation_resource: Model,
    name: &str,
    machine: &mut Machine,
    scene: &mut Scene,
    model: Handle<Node>,
) -> (Handle<Animation>, Handle<State>) {
    let animation = *animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    let node = machine.add_node(PoseNode::make_play_animation(animation));
    let state = machine.add_state(State::new(name, node));
    (animation, state)
}

pub struct ModelMap {
    pub map: HashMap<String, Model>,
}

impl ModelMap {
    pub async fn new<I>(paths: I, resource_manager: ResourceManager) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        Self {
            map: rg3d::futures::future::join_all(
                paths
                    .into_iter()
                    .map(|path| resource_manager.request_model(path))
                    .collect::<Vec<_>>(),
            )
            .await
            .into_iter()
            .map(|r| {
                let resource = r.unwrap();
                let key = resource.state().path().to_string_lossy().into_owned();
                (key, resource)
            })
            .collect::<HashMap<_, _>>(),
        }
    }
}

impl<T: AsRef<str>> Index<T> for ModelMap {
    type Output = Model;

    fn index(&self, index: T) -> &Self::Output {
        self.map.get(index.as_ref()).unwrap()
    }
}

fn vector_to_quat(vec: Vector3<f32>) -> UnitQuaternion<f32> {
    let dot = vec.normalize().dot(&Vector3::y());

    if dot.abs() > 1.0 - 10.0 * std::f32::EPSILON {
        // Handle singularity when normal of impact point is collinear with Y axis.
        UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -dot.signum() * 90.0f32.to_radians())
    } else {
        UnitQuaternion::face_towards(&vec, &Vector3::y())
    }
}

pub struct Game {
    menu: Menu,
    engine: GameEngine,
    level: Option<Level>,
    debug_text: UiNodeHandle,
    debug_string: String,
    last_tick_time: time::Instant,
    running: bool,
    control_scheme: Arc<RwLock<ControlScheme>>,
    time: GameTime,
    events_receiver: Receiver<Message>,
    events_sender: Sender<Message>,
    load_context: Option<Arc<Mutex<LoadContext>>>,
    loading_screen: LoadingScreen,
    death_screen: DeathScreen,
    weapon_display: WeaponDisplay,
    inventory_interface: InventoryInterface,
    item_display: ItemDisplay,
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

#[repr(u16)]
pub enum CollisionGroups {
    ActorCapsule = 1 << 0,
    All = std::u16::MAX,
}

pub struct LoadContext {
    level: Option<(Level, Scene)>,
}

impl Game {
    pub fn run() {
        let events_loop = EventLoop::<()>::new();

        let inner_size = if let Some(primary_monitor) = events_loop.primary_monitor() {
            let mut monitor_dimensions = primary_monitor.size();
            monitor_dimensions.height = (monitor_dimensions.height as f32 * 0.7) as u32;
            monitor_dimensions.width = (monitor_dimensions.width as f32 * 0.7) as u32;
            monitor_dimensions.to_logical::<f32>(primary_monitor.scale_factor())
        } else {
            LogicalSize::new(1024.0, 768.0)
        };

        let font = SharedFont(Arc::new(Mutex::new(
            Font::from_file(
                Path::new("data/ui/SquaresBold.ttf"),
                31.0,
                Font::default_char_set(),
            )
            .unwrap(),
        )));

        let smaller_font = SharedFont(Arc::new(Mutex::new(
            Font::from_file(
                Path::new("data/ui/SquaresBold.ttf"),
                20.0,
                Font::default_char_set(),
            )
            .unwrap(),
        )));

        let window_builder = rg3d::window::WindowBuilder::new()
            .with_title("Station Iapetus")
            .with_inner_size(inner_size)
            .with_resizable(true);

        let mut engine = GameEngine::new(window_builder, &events_loop, false).unwrap();

        let mut control_scheme = ControlScheme::default();

        match Config::load() {
            Ok(config) => {
                match engine
                    .renderer
                    .set_quality_settings(&config.graphics_settings)
                {
                    Ok(_) => {
                        Log::writeln(
                            MessageKind::Information,
                            "Graphics settings were applied correctly!".to_string(),
                        );
                    }
                    Err(e) => Log::writeln(
                        MessageKind::Error,
                        format!("Failed to set graphics settings. Reason: {:?}", e),
                    ),
                }

                control_scheme = config.controls;
            }
            Err(e) => {
                Log::writeln(
                    MessageKind::Error,
                    format!(
                        "Failed to load config. Recovering to default values... Reason: {:?}",
                        e
                    ),
                );
            }
        }

        let control_scheme = Arc::new(RwLock::new(control_scheme));

        let fixed_timestep = 1.0 / FIXED_FPS;

        let time = GameTime {
            clock: Instant::now(),
            elapsed: 0.0,
            delta: fixed_timestep,
        };

        let (tx, rx) = mpsc::channel();

        let mut game = Game {
            loading_screen: LoadingScreen::new(
                &mut engine.user_interface.build_ctx(),
                inner_size.width,
                inner_size.height,
            ),
            running: true,
            menu: rg3d::futures::executor::block_on(Menu::new(
                &mut engine,
                control_scheme.clone(),
                tx.clone(),
                font.clone(),
            )),
            death_screen: DeathScreen::new(&mut engine.user_interface, font.clone(), tx.clone()),
            control_scheme,
            debug_text: Handle::NONE,
            weapon_display: WeaponDisplay::new(font, engine.resource_manager.clone()),
            item_display: ItemDisplay::new(smaller_font),
            engine,
            level: None,
            debug_string: String::new(),
            last_tick_time: time::Instant::now(),
            time,
            load_context: None,
            inventory_interface: InventoryInterface::new(tx.clone()),
            events_receiver: rx,
            events_sender: tx,
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
                            game.handle_ui_message(&ui_event);
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
                    game.render(fixed_timestep);
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

    fn handle_ui_message(&mut self, message: &GuiMessage) {
        self.menu
            .handle_ui_message(&mut self.engine, self.level.as_ref(), &message);

        self.death_screen.handle_ui_message(message);

        if matches!(message.data(), UiMessageData::Button(ButtonMessage::Click))
            || (matches!(
                message.data(),
                UiMessageData::CheckBox(CheckBoxMessage::Check(_))
            ) && message.direction() == MessageDirection::FromWidget)
        {
            self.events_sender
                .send(Message::Play2DSound {
                    path: PathBuf::from("data/sounds/click.ogg"),
                    gain: 0.8,
                })
                .unwrap();
        }
    }

    fn render(&mut self, delta: f32) {
        self.engine
            .renderer
            .render_ui_to_texture(
                self.weapon_display.render_target.clone(),
                &mut self.weapon_display.ui,
            )
            .unwrap();

        self.engine
            .renderer
            .render_ui_to_texture(
                self.inventory_interface.render_target.clone(),
                &mut self.inventory_interface.ui,
            )
            .unwrap();

        self.engine
            .renderer
            .render_ui_to_texture(
                self.item_display.render_target.clone(),
                &mut self.item_display.ui,
            )
            .unwrap();

        self.engine.render(delta).unwrap();
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

        Log::writeln(
            MessageKind::Information,
            "Game state successfully loaded!".to_owned(),
        );

        // Hide menu only of we successfully loaded a save.
        self.set_menu_visible(false);
        self.death_screen
            .set_visible(&self.engine.user_interface, false);

        // Set control scheme for player.
        if let Some(level) = &mut self.level {
            level.resolve(
                &mut self.engine,
                self.events_sender.clone(),
                self.control_scheme.clone(),
                self.weapon_display.render_target.clone(),
                self.inventory_interface.render_target.clone(),
                self.item_display.render_target.clone(),
            );
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
        self.menu.set_visible(&mut self.engine, false);

        let resource_manager = self.engine.resource_manager.clone();
        let control_scheme = self.control_scheme.clone();
        let sender = self.events_sender.clone();
        let display_texture = self.weapon_display.render_target.clone();
        let inventory_texture = self.inventory_interface.render_target.clone();
        let item_texture = self.item_display.render_target.clone();

        std::thread::spawn(move || {
            let level = rg3d::futures::executor::block_on(Level::new(
                resource_manager,
                control_scheme,
                sender,
                display_texture,
                inventory_texture,
                item_texture,
            ));

            ctx.lock().unwrap().level = Some(level);
        });
    }

    pub fn set_menu_visible(&mut self, visible: bool) {
        self.menu.set_visible(&mut self.engine, visible);
    }

    pub fn is_any_menu_visible(&self) -> bool {
        self.menu.is_visible(&self.engine.user_interface)
            || self.death_screen.is_visible(&self.engine.user_interface)
    }

    pub fn update(&mut self, time: GameTime) {
        let window = self.engine.get_window();

        window.set_cursor_visible(self.is_any_menu_visible());
        let _ = window.set_cursor_grab(!self.is_any_menu_visible());

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
                    self.menu.sync_to_model(&mut self.engine, true);
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

        if let Some(ref mut level) = self.level {
            let menu_visible = self.menu.is_visible(&self.engine.user_interface);
            if !menu_visible {
                level.update(&mut self.engine, time);
                let player = level.get_player();
                if player.is_some() {
                    if let Actor::Player(player) = level.actors().get(player) {
                        self.weapon_display.sync_to_model(player, level.weapons());
                    }
                }
            }
            self.engine.scenes[level.scene].enabled = !menu_visible;
        }

        self.menu.scene.update(&mut self.engine, time.delta);
        self.weapon_display.update(time.delta);
        self.inventory_interface.update(time.delta);
        self.item_display.update(time.delta);
        self.engine.update(time.delta);

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
                    self.death_screen
                        .set_visible(&self.engine.user_interface, true);
                    self.menu.sync_to_model(&mut self.engine, false);
                }
                Message::SetMusicVolume { volume } => {
                    self.engine.scenes[self.menu.scene.scene]
                        .sound_context
                        .state()
                        .source_mut(self.menu.scene.music)
                        .set_gain(*volume);
                }
                Message::ToggleMainMenu => {
                    self.menu.set_visible(&mut self.engine, true);
                    self.death_screen
                        .set_visible(&self.engine.user_interface, false);
                }
                Message::SyncInventory => {
                    if let Some(ref mut level) = self.level {
                        if let Actor::Player(player) = level.actors().get(level.get_player()) {
                            self.inventory_interface
                                .sync_to_model(self.engine.resource_manager.clone(), player);
                        }
                    }
                }
                &Message::ShowItemDisplay { item, count } => {
                    self.item_display.sync_to_model(
                        self.engine.resource_manager.clone(),
                        item,
                        count,
                    );
                }
                Message::Play2DSound { path, gain } => {
                    if let Ok(buffer) = rg3d::futures::executor::block_on(
                        self.engine
                            .resource_manager
                            .request_sound_buffer(path, false),
                    ) {
                        if let Ok(shot_sound) = GenericSourceBuilder::new(buffer.into())
                            .with_status(Status::Playing)
                            .with_play_once(true)
                            .with_gain(*gain)
                            .build_source()
                        {
                            let mut state = self.engine.scenes[self.menu.scene.scene]
                                .sound_context
                                .state();
                            state.add_source(shot_sound);
                        }
                    }
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
        write!(
            self.debug_string,
            "Up time: {}\n{}{}",
            elapsed,
            self.engine.renderer.get_statistics(),
            if let Some(level) = self.level.as_ref() {
                self.engine.scenes[level.scene].performance_statistics
            } else {
                Default::default()
            }
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
                if let Some(level) = self.level.as_mut() {
                    let player_handle = level.get_player();
                    let player =
                        if let Actor::Player(player) = level.actors_mut().get_mut(player_handle) {
                            player
                        } else {
                            unreachable!()
                        };
                    self.inventory_interface.process_os_event(
                        &event,
                        &self.control_scheme.read().unwrap(),
                        player_handle,
                        player,
                    );
                }
            }
        }

        if !self.is_any_menu_visible() {
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

        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { input, .. },
            ..
        } = event
        {
            if let ElementState::Pressed = input.state {
                if let Some(key) = input.virtual_keycode {
                    if key == VirtualKeyCode::Escape && self.level.is_some() {
                        self.set_menu_visible(!self.is_any_menu_visible());
                    }
                }
            }
        }

        self.menu.process_input_event(&mut self.engine, &event);
    }
}

fn main() {
    Game::run();
}
