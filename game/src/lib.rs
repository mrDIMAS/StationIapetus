#![allow(clippy::too_many_arguments)]

pub mod bot;
pub mod character;
pub mod config;
pub mod control_scheme;
pub mod door;
pub mod effects;
pub mod elevator;
pub mod gui;
pub mod highlight;
pub mod inventory;
pub mod level;
pub mod light;
pub mod loading_screen;
pub mod menu;
pub mod message;
pub mod options_menu;
pub mod player;
pub mod sound;
pub mod utils;
pub mod weapon;

use crate::{
    bot::Bot,
    config::{Config, SoundConfig},
    control_scheme::ControlScheme,
    door::Door,
    effects::{beam::Beam, rail::Rail},
    elevator::{call_button::CallButton, Elevator},
    gui::{
        inventory::InventoryInterface, item_display::ItemDisplay, journal::JournalDisplay,
        weapon_display::WeaponDisplay, DeathScreen, FinalScreen,
    },
    highlight::HighlightRenderPass,
    level::{
        arrival::enemy_trap::EnemyTrap, death_zone::DeathZone, decal::Decal, explosion::Explosion,
        item::Item, spawn::CharacterSpawnPoint, trigger::Trigger, turret::Turret, Level,
    },
    light::AnimatedLight,
    loading_screen::LoadingScreen,
    menu::Menu,
    message::Message,
    player::{camera::CameraController, Player},
    utils::use_hrtf,
    weapon::{kinetic::KineticGun, projectile::Projectile, sight::LaserSight, Weapon},
};
use fyrox::{
    core::{
        color::Color,
        futures::executor::block_on,
        log::{Log, MessageKind},
        parking_lot::Mutex,
        pool::Handle,
        sstorage::ImmutableString,
        visitor::{Visit, VisitResult, Visitor},
    },
    dpi::LogicalSize,
    engine::GraphicsContext,
    event::{ElementState, Event, WindowEvent},
    gui::{
        button::ButtonMessage,
        check_box::CheckBoxMessage,
        font::Font,
        message::{MessageDirection, UiMessage},
        text::{TextBuilder, TextMessage},
        widget::{WidgetBuilder, WidgetMessage},
        UiNode, UserInterface,
    },
    keyboard::KeyCode,
    material::{shader::SamplerFallback, Material, PropertyValue},
    plugin::{Plugin, PluginContext, PluginRegistrationContext},
    renderer::framework::gpu_texture::PixelKind,
    resource::texture::TextureResource,
    scene::{
        base::BaseBuilder,
        sound::{SoundBuffer, SoundBuilder, Status},
        Scene,
    },
    utils::translate_event,
    window::CursorGrabMode,
};
use std::{
    cell::RefCell,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

#[derive(Visit)]
pub struct Game {
    menu: Menu,
    level: Option<Level>,
    debug_text: Handle<UiNode>,
    debug_string: String,
    running: bool,
    #[visit(skip)]
    control_scheme: ControlScheme,
    #[visit(skip)]
    message_receiver: Receiver<Message>,
    #[visit(skip)]
    message_sender: MessageSender,
    loading_screen: LoadingScreen,
    death_screen: DeathScreen,
    final_screen: FinalScreen,
    weapon_display: WeaponDisplay,
    inventory_interface: InventoryInterface,
    item_display: ItemDisplay,
    journal_display: JournalDisplay,
    // We're storing sound config separately because we can adjust sound
    // setting in the options but don't have a level loaded. This field
    // is data-model for options menu.
    sound_config: SoundConfig,
    show_debug_info: bool,
    #[visit(skip)]
    highlighter: Option<Rc<RefCell<HighlightRenderPass>>>,
}

impl Default for Game {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            menu: Default::default(),
            level: None,
            debug_text: Default::default(),
            debug_string: Default::default(),
            running: Default::default(),
            control_scheme: Default::default(),
            message_receiver: rx,
            message_sender: MessageSender { sender: tx },
            loading_screen: Default::default(),
            death_screen: Default::default(),
            final_screen: Default::default(),
            weapon_display: Default::default(),
            inventory_interface: Default::default(),
            item_display: Default::default(),
            journal_display: Default::default(),
            sound_config: Default::default(),
            show_debug_info: Default::default(),
            highlighter: Default::default(),
        }
    }
}

#[repr(u16)]
pub enum CollisionGroups {
    ActorCapsule = 1 << 0,
    All = std::u16::MAX,
}

pub fn create_display_material(display_texture: TextureResource) -> Arc<Mutex<Material>> {
    let mut material = Material::standard();

    Log::verify(material.set_property(
        &ImmutableString::new("diffuseTexture"),
        PropertyValue::Sampler {
            value: Some(display_texture),
            fallback: SamplerFallback::White,
        },
    ));

    Arc::new(Mutex::new(material))
}

#[derive(Clone)]
pub struct MessageSender {
    sender: Sender<Message>,
}

impl MessageSender {
    pub fn send(&self, message: Message) {
        Log::verify(self.sender.send(message))
    }
}

impl Game {
    fn handle_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        self.menu.handle_ui_message(
            context,
            message,
            &mut self.control_scheme,
            &mut self.show_debug_info,
            &self.sound_config,
            &self.message_sender,
        );

        self.death_screen
            .handle_ui_message(message, &self.message_sender);
        self.final_screen
            .handle_ui_message(message, &self.message_sender);

        let play_sound = if message.direction() == MessageDirection::FromWidget {
            if let Some(ButtonMessage::Click) = message.data() {
                true
            } else {
                matches!(message.data(), Some(CheckBoxMessage::Check(_)))
            }
        } else {
            false
        };

        if play_sound {
            self.message_sender.send(Message::Play2DSound {
                path: PathBuf::from("data/sounds/click.ogg"),
                gain: 0.8,
            });
        }
    }

    fn render_offscreen(&mut self, context: &mut PluginContext) {
        if let GraphicsContext::Initialized(ref mut graphics_context) = context.graphics_context {
            let renderer = &mut graphics_context.renderer;

            for (rt, ui) in [
                (
                    self.weapon_display.render_target.clone(),
                    &mut self.weapon_display.ui,
                ),
                (
                    self.inventory_interface.render_target.clone(),
                    &mut self.inventory_interface.ui,
                ),
                (
                    self.item_display.render_target.clone(),
                    &mut self.item_display.ui,
                ),
                (
                    self.journal_display.render_target.clone(),
                    &mut self.journal_display.ui,
                ),
            ] {
                Log::verify(renderer.render_ui_to_texture(
                    rt,
                    ui.screen_size(),
                    ui.draw(),
                    Color::TRANSPARENT,
                    PixelKind::SRGBA8,
                ));
            }
        }
    }

    fn debug_render(&mut self, context: &mut PluginContext) {
        if let Some(level) = self.level.as_mut() {
            level.debug_draw(context);
        }
    }

    pub fn create_debug_ui(&mut self, context: &mut PluginContext) {
        self.debug_text = TextBuilder::new(WidgetBuilder::new().with_width(400.0))
            .build(&mut context.user_interfaces.first_mut().build_ctx());
    }

    pub fn save_game(&mut self, context: &mut PluginContext) -> VisitResult {
        if let Some(level) = self.level.as_mut() {
            let mut visitor = Visitor::new();

            context.scenes[level.scene].save("Scene", &mut visitor)?;
            level.visit("Level", &mut visitor)?;

            // Debug output
            if let Ok(mut file) = File::create(Path::new("save.txt")) {
                file.write_all(visitor.save_text().as_bytes()).unwrap();
            }

            visitor.save_binary(Path::new("save.rgs"))
        } else {
            Ok(())
        }
    }

    pub fn load_game(&mut self, context: &mut PluginContext) {
        context.async_scene_loader.request_raw("save.rgs");
    }

    fn destroy_level(&mut self, context: &mut PluginContext) {
        if let Some(ref mut level) = self.level.take() {
            level.destroy(context);
            Log::info("Current level destroyed!");
        }
    }

    pub fn load_level(&mut self, path: PathBuf, context: &mut PluginContext) {
        self.destroy_level(context);
        context.async_scene_loader.request(path);
    }

    pub fn set_menu_visible(&mut self, visible: bool, context: &mut PluginContext) {
        self.menu.set_visible(context, visible);
    }

    pub fn is_any_menu_visible(&self, context: &PluginContext) -> bool {
        let ui = context.user_interfaces.first();
        self.menu.is_visible(ui)
            || self.death_screen.is_visible(ui)
            || self.final_screen.is_visible(ui)
    }

    pub fn update(&mut self, ctx: &mut PluginContext) {
        if let GraphicsContext::Initialized(ref graphics_context) = ctx.graphics_context {
            let window = &graphics_context.window;
            window.set_cursor_visible(self.is_any_menu_visible(ctx));
            let _ = window.set_cursor_grab(if !self.is_any_menu_visible(ctx) {
                CursorGrabMode::Confined
            } else {
                CursorGrabMode::None
            });
        }

        let ui = ctx.user_interfaces.first();

        self.loading_screen.set_progress(
            ui,
            ctx.resource_manager.state().loading_progress() as f32 / 100.0,
        );

        if let Some(ref mut level) = self.level {
            ctx.scenes[level.scene]
                .enabled
                .set_value_silent(!self.menu.is_visible(ui));
        }

        self.weapon_display.update(ctx.dt);
        self.inventory_interface.update(ctx.dt);
        self.item_display.update(ctx.dt);

        for scene in ctx.scenes.iter_mut() {
            scene
                .graph
                .sound_context
                .state()
                .bus_graph_mut()
                .primary_bus_mut()
                .set_gain(self.sound_config.master_volume);
        }

        self.handle_messages(ctx);

        self.update_statistics(0.0, ctx);

        // <<<<<<<<< ENABLE THIS FOR DEBUGGING
        if false {
            self.debug_render(ctx);
        }
    }

    fn handle_messages(&mut self, context: &mut PluginContext) {
        while let Ok(message) = self.message_receiver.try_recv() {
            match &message {
                Message::StartNewGame => {
                    self.load_level(Level::ARRIVAL_PATH.into(), context);
                }
                Message::LoadTestbed => {
                    self.load_level(Level::TESTBED_PATH.into(), context);
                }
                Message::SaveGame => match self.save_game(context) {
                    Ok(_) => Log::info("Successfully saved"),
                    Err(e) => Log::err(format!("Failed to make a save, reason: {e}")),
                },
                Message::LoadGame => {
                    self.load_game(context);
                }
                Message::LoadLevel { path } => self.load_level(path.clone(), context),
                Message::QuitGame => {
                    self.destroy_level(context);
                    self.running = false;
                }
                Message::EndMatch => {
                    self.destroy_level(context);
                    self.death_screen
                        .set_visible(context.user_interfaces.first(), true);
                    self.menu.sync_to_model(context, false);
                }
                Message::EndGame => {
                    self.destroy_level(context);
                    self.final_screen
                        .set_visible(context.user_interfaces.first(), true);
                    self.menu.sync_to_model(context, false);
                }
                Message::SetMusicVolume(volume) => {
                    self.sound_config.music_volume = *volume;
                    // TODO: Apply to sound manager of level when it will handle music!
                    context.scenes[self.menu.scene.scene].graph[self.menu.scene.music]
                        .as_sound_mut()
                        .set_gain(*volume);
                }
                Message::SetUseHrtf(state) => {
                    self.sound_config.use_hrtf = *state;
                    // Hrtf is applied **only** to game scene!
                    if let Some(level) = self.level.as_ref() {
                        let scene = &mut context.scenes[level.scene];
                        if self.sound_config.use_hrtf {
                            block_on(use_hrtf(
                                &mut scene.graph.sound_context,
                                context.resource_manager,
                            ))
                        } else {
                            scene
                                .graph
                                .sound_context
                                .state()
                                .set_renderer(fyrox::scene::sound::Renderer::Default);
                        }
                    }
                }
                Message::SetMasterVolume(volume) => {
                    self.sound_config.master_volume = *volume;
                }
                Message::SaveConfig => {
                    match Config::save(
                        context,
                        self.control_scheme.clone(),
                        self.sound_config.clone(),
                        self.show_debug_info,
                    ) {
                        Ok(_) => {
                            Log::info("Settings saved!");
                        }
                        Err(e) => Log::writeln(
                            MessageKind::Error,
                            format!("Failed to save settings. Reason: {e:?}"),
                        ),
                    }
                }
                Message::ToggleMainMenu => {
                    self.menu.set_visible(context, true);
                    let ui = context.user_interfaces.first();
                    self.death_screen.set_visible(ui, false);
                    self.final_screen.set_visible(ui, false);
                }
                Message::SyncInventory => {
                    if let Some(ref mut level) = self.level {
                        let player_ref = context.scenes[level.scene].graph[level.player]
                            .try_get_script::<Player>()
                            .unwrap();
                        self.inventory_interface.sync_to_model(player_ref);
                    }
                }
                Message::SyncJournal => {
                    if let Some(ref mut level) = self.level {
                        let player_ref = context.scenes[level.scene].graph[level.player]
                            .try_get_script::<Player>()
                            .unwrap();
                        self.journal_display.sync_to_model(&player_ref.journal);
                    }
                }
                Message::Play2DSound { path, gain } => {
                    if let Ok(buffer) = fyrox::core::futures::executor::block_on(
                        context.resource_manager.request::<SoundBuffer>(path),
                    ) {
                        let menu_scene = &mut context.scenes[self.menu.scene.scene];
                        SoundBuilder::new(BaseBuilder::new())
                            .with_buffer(buffer.into())
                            .with_status(Status::Playing)
                            .with_play_once(true)
                            .with_gain(*gain)
                            .build(&mut menu_scene.graph);
                    }
                }
            }
        }
    }

    pub fn update_statistics(&mut self, elapsed: f64, context: &mut PluginContext) {
        let ui = context.user_interfaces.first_mut();

        if self.show_debug_info {
            if let GraphicsContext::Initialized(ref graphics_context) = context.graphics_context {
                self.debug_string.clear();
                use std::fmt::Write;
                write!(
                    self.debug_string,
                    "Up time: {:.1}\n{}{}\n{}",
                    elapsed,
                    graphics_context.renderer.get_statistics(),
                    if let Some(level) = self.level.as_ref() {
                        context.scenes[level.scene].performance_statistics.clone()
                    } else {
                        Default::default()
                    },
                    context.performance_statistics,
                )
                .unwrap();

                ui.send_message(TextMessage::text(
                    self.debug_text,
                    MessageDirection::ToWidget,
                    self.debug_string.clone(),
                ));
            }
        }

        ui.send_message(WidgetMessage::visibility(
            self.debug_text,
            MessageDirection::ToWidget,
            self.show_debug_info,
        ));
    }

    fn process_dispatched_event(&mut self, event: &Event<()>, context: &mut PluginContext) {
        if let Event::WindowEvent { event, .. } = event {
            if let Some(event) = translate_event(event) {
                if let Some(level) = self.level.as_mut() {
                    let player_handle = level.get_player();
                    if let Some(player_ref) =
                        context.scenes.try_get_mut(level.scene).and_then(|s| {
                            s.graph
                                .try_get_mut(player_handle)
                                .and_then(|p| p.try_get_script_mut::<Player>())
                        })
                    {
                        self.inventory_interface.process_os_event(
                            &event,
                            &self.control_scheme,
                            player_ref,
                            player_handle,
                            &self.message_sender,
                        );
                        self.journal_display
                            .process_os_event(&event, &self.control_scheme);
                    }
                }
            }
        }
    }

    pub fn on_window_resized(
        &mut self,
        ui: &UserInterface,
        graphics_context: &mut GraphicsContext,
        width: f32,
        height: f32,
    ) {
        self.loading_screen.resize(ui, width, height);
        self.death_screen.resize(ui, width, height);
        self.create_highlighter(graphics_context, width as usize, height as usize);
    }

    fn create_highlighter(
        &mut self,
        graphics_context: &mut GraphicsContext,
        width: usize,
        height: usize,
    ) {
        if let GraphicsContext::Initialized(graphics_context) = graphics_context {
            if let Some(highlighter) = self.highlighter.as_ref() {
                graphics_context
                    .renderer
                    .remove_render_pass(highlighter.clone());
            }

            let highlighter =
                HighlightRenderPass::new(&graphics_context.renderer.state, width, height);

            if let Some(level) = self.level.as_ref() {
                highlighter.borrow_mut().scene_handle = level.scene;
            }

            graphics_context
                .renderer
                .add_render_pass(highlighter.clone());

            self.highlighter = Some(highlighter);
        }
    }

    pub fn process_input_event(&mut self, event: &Event<()>, context: &mut PluginContext) {
        self.process_dispatched_event(event, context);

        if let Event::WindowEvent {
            event: WindowEvent::KeyboardInput { event: input, .. },
            ..
        } = event
        {
            if let ElementState::Pressed = input.state {
                if input.physical_key == KeyCode::Escape && self.level.is_some() {
                    self.set_menu_visible(!self.is_any_menu_visible(context), context);
                }
            }
        }

        self.menu
            .process_input_event(context, event, &mut self.control_scheme);
    }
}

impl Plugin for Game {
    fn register(&self, context: PluginRegistrationContext) {
        context
            .serialization_context
            .script_constructors
            .add::<Door>("Door")
            .add::<Turret>("Turret")
            .add::<Weapon>("Weapon")
            .add::<Item>("Item")
            .add::<Decal>("Decal")
            .add::<Player>("Player")
            .add::<CameraController>("Camera Controller")
            .add::<Bot>("Bot")
            .add::<CharacterSpawnPoint>("Character Spawn Point")
            .add::<DeathZone>("Death Zone")
            .add::<AnimatedLight>("Animated Light")
            .add::<Elevator>("Elevator")
            .add::<CallButton>("Call Button")
            .add::<Projectile>("Projectile")
            .add::<LaserSight>("LaserSight")
            .add::<Rail>("Rail")
            .add::<Explosion>("Explosion")
            .add::<Beam>("Beam")
            .add::<KineticGun>("KineticGun")
            .add::<EnemyTrap>("ArrivalEnemyTrap")
            .add::<Trigger>("Trigger");
    }

    fn init(&mut self, scene_path: Option<&str>, mut context: PluginContext) {
        if let Some(scene_path) = scene_path {
            context.async_scene_loader.request(scene_path);
        }

        let font = context
            .resource_manager
            .request::<Font>(Path::new("data/ui/SquaresBold.ttf"));

        let mut control_scheme = ControlScheme::default();
        let mut sound_config = SoundConfig::default();
        let mut show_debug_info = false;

        match Config::load() {
            Ok(config) => {
                show_debug_info = config.show_debug_info;
                sound_config = config.sound;
                control_scheme = config.controls;
            }
            Err(e) => {
                Log::writeln(
                    MessageKind::Error,
                    format!("Failed to load config. Recovering to default values... Reason: {e:?}"),
                );
            }
        }

        let (tx, rx) = mpsc::channel();

        let message_sender = MessageSender { sender: tx };
        let weapon_display = WeaponDisplay::new(font.clone(), context.resource_manager.clone());
        let inventory_interface = InventoryInterface::new();
        let item_display = ItemDisplay::new(font.clone());
        let journal_display = JournalDisplay::new();

        *self = Game {
            show_debug_info,
            loading_screen: LoadingScreen::new(
                &mut context.user_interfaces.first_mut().build_ctx(),
            ),
            running: true,
            menu: fyrox::core::futures::executor::block_on(Menu::new(
                &mut context,
                &control_scheme,
                font.clone(),
                show_debug_info,
                &sound_config,
            )),
            death_screen: DeathScreen::new(context.user_interfaces.first_mut(), font.clone()),
            final_screen: FinalScreen::new(context.user_interfaces.first_mut(), font),
            control_scheme,
            debug_text: Handle::NONE,
            weapon_display,
            item_display,
            journal_display,
            level: None,
            debug_string: String::new(),
            inventory_interface,
            message_receiver: rx,
            message_sender,
            sound_config,
            highlighter: None,
        };

        self.create_debug_ui(&mut context);
        self.menu.set_visible(&mut context, true);
    }

    fn update(&mut self, ctx: &mut PluginContext) {
        self.update(ctx);

        if !self.running {
            ctx.window_target.unwrap().exit();
        }
    }

    fn on_os_event(&mut self, event: &Event<()>, mut ctx: PluginContext) {
        self.process_input_event(event, &mut ctx);

        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => {
                    self.destroy_level(&mut ctx);
                    ctx.window_target.unwrap().exit();
                }
                WindowEvent::Resized(new_size) => self.on_window_resized(
                    ctx.user_interfaces.first(),
                    ctx.graphics_context,
                    new_size.width as f32,
                    new_size.height as f32,
                ),
                _ => (),
            }
        }
    }

    fn on_graphics_context_initialized(&mut self, mut context: PluginContext) {
        let graphics_context = context.graphics_context.as_initialized_mut();

        let inner_size = if let Some(primary_monitor) = graphics_context.window.primary_monitor() {
            let mut monitor_dimensions = primary_monitor.size();
            monitor_dimensions.height = (monitor_dimensions.height as f32 * 0.7) as u32;
            monitor_dimensions.width = (monitor_dimensions.width as f32 * 0.7) as u32;
            monitor_dimensions.to_logical::<f32>(primary_monitor.scale_factor())
        } else {
            LogicalSize::new(1024.0, 768.0)
        };

        let window = &graphics_context.window;
        window.set_title("Station Iapetus");
        window.set_resizable(true);
        let _ = window.request_inner_size(inner_size);

        // Re-load config here as well.
        match Config::load() {
            Ok(config) => {
                match graphics_context
                    .renderer
                    .set_quality_settings(&config.graphics_settings)
                {
                    Ok(_) => {
                        Log::warn("Graphics settings were applied correctly!");
                    }
                    Err(e) => Log::err(format!("Failed to set graphics settings. Reason: {e:?}")),
                }
            }
            Err(e) => {
                Log::writeln(
                    MessageKind::Error,
                    format!("Failed to load config. Recovering to default values... Reason: {e:?}"),
                );
            }
        }

        self.menu
            .on_graphics_context_initialized(context.user_interfaces.first_mut(), graphics_context);

        self.on_window_resized(
            context.user_interfaces.first(),
            context.graphics_context,
            inner_size.width,
            inner_size.height,
        );

        self.menu.sync_to_model(&mut context, self.level.is_some());
    }

    fn before_rendering(&mut self, mut context: PluginContext) {
        self.render_offscreen(&mut context);
    }

    fn on_ui_message(&mut self, context: &mut PluginContext, message: &UiMessage) {
        self.handle_ui_message(context, message);
    }

    fn on_scene_begin_loading(&mut self, _path: &Path, ctx: &mut PluginContext) {
        self.destroy_level(ctx);
        let ui = ctx.user_interfaces.first();
        self.death_screen.set_visible(ui, false);
        self.final_screen.set_visible(ui, false);

        ui.send_message(WidgetMessage::visibility(
            self.loading_screen.root,
            MessageDirection::ToWidget,
            true,
        ));

        self.menu.set_visible(ctx, false);
    }

    fn on_scene_loaded(
        &mut self,
        _path: &Path,
        scene: Handle<Scene>,
        data: &[u8],
        ctx: &mut PluginContext,
    ) {
        if let Some(highlighter) = self.highlighter.as_mut() {
            highlighter.borrow_mut().scene_handle = scene;
        }

        if let Ok(mut visitor) = Visitor::load_from_memory(data) {
            let mut level = Level::default();
            if level.visit("Level", &mut visitor).is_ok() {
                // Means that we're loading a saved game.
                level.scene = scene;
                level.resolve(ctx, self.message_sender.clone());
                self.level = Some(level);
            } else {
                self.level = Some(Level::from_existing_scene(
                    &mut ctx.scenes[scene],
                    scene,
                    self.message_sender.clone(),
                    self.sound_config.clone(),
                    ctx.resource_manager.clone(),
                ));
            }
        }

        self.set_menu_visible(false, ctx);
        ctx.user_interfaces
            .first()
            .send_message(WidgetMessage::visibility(
                self.loading_screen.root,
                MessageDirection::ToWidget,
                false,
            ));
        self.menu.sync_to_model(ctx, true);

        Log::info("Level was loaded successfully!");
    }
}
