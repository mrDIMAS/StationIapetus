use crate::{
    control_scheme::ControlScheme, gui::Gui, gui::GuiMessage, gui::UiNode, level::Level,
    message::Message, options_menu::OptionsMenu, utils::create_camera, GameEngine,
};
use rg3d::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
    },
    event::{Event, WindowEvent},
    gui::{
        button::ButtonBuilder,
        grid::{Column, GridBuilder, Row},
        message::{ButtonMessage, MessageDirection, UiMessageData, WidgetMessage, WindowMessage},
        ttf::SharedFont,
        widget::WidgetBuilder,
        window::{WindowBuilder, WindowTitle},
        Thickness,
    },
    scene::{node::Node, Scene},
    sound::source::{generic::GenericSourceBuilder, SoundSource, Status},
};
use std::sync::mpsc::Sender;

pub struct Menu {
    pub scene: MenuScene,
    sender: Sender<Message>,
    root: Handle<UiNode>,
    btn_new_game: Handle<UiNode>,
    btn_save_game: Handle<UiNode>,
    btn_settings: Handle<UiNode>,
    btn_load_game: Handle<UiNode>,
    btn_quit_game: Handle<UiNode>,
    options_menu: OptionsMenu,
}

pub struct MenuScene {
    pub scene: Handle<Scene>,
    iapetus: Handle<Node>,
    angle: f32,
    pub music: Handle<SoundSource>,
}

impl MenuScene {
    pub async fn new(engine: &mut GameEngine) -> Self {
        let mut scene = Scene::from_file("data/levels/menu.rgs", engine.resource_manager.clone())
            .await
            .unwrap();

        scene.ambient_lighting_color = Color::opaque(20, 20, 20);

        let buffer = engine
            .resource_manager
            .request_sound_buffer(
                "data/music/Pura Sombar - Tongues falling from an opened sky.ogg",
                true,
            )
            .await
            .unwrap();

        let music = scene.sound_context.state().add_source(
            GenericSourceBuilder::new(buffer.into())
                .with_looping(true)
                .with_status(Status::Playing)
                .with_gain(0.5)
                .build_source()
                .unwrap(),
        );

        let position = scene.graph[scene
            .graph
            .find_from_root(&mut |n| n.tag() == "CameraPoint")]
        .global_position();

        create_camera(
            engine.resource_manager.clone(),
            position,
            &mut scene.graph,
            200.0,
        )
        .await;

        Self {
            music,
            angle: 0.0,
            iapetus: scene.graph.find_from_root(&mut |n| n.tag() == "Iapetus"),
            scene: engine.scenes.add(scene),
        }
    }

    pub fn update(&mut self, engine: &mut GameEngine, dt: f32) {
        let scene = &mut engine.scenes[self.scene];

        self.angle += 0.18 * dt;

        scene.graph[self.iapetus]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(
                &Vector3::y_axis(),
                self.angle.to_radians(),
            ));
    }
}

impl Menu {
    pub async fn new(
        engine: &mut GameEngine,
        control_scheme: &ControlScheme,
        sender: Sender<Message>,
        font: SharedFont,
    ) -> Self {
        let frame_size = engine.renderer.get_frame_size();

        let scene = MenuScene::new(engine).await;

        let ctx = &mut engine.user_interface.build_ctx();

        let btn_new_game;
        let btn_settings;
        let btn_save_game;
        let btn_load_game;
        let btn_quit_game;
        let root: Handle<UiNode> = GridBuilder::new(
            WidgetBuilder::new()
                .with_width(frame_size.0 as f32)
                .with_height(frame_size.1 as f32)
                .with_child(
                    WindowBuilder::new(WidgetBuilder::new().on_row(1).on_column(0))
                        .can_resize(false)
                        .can_minimize(false)
                        .can_close(false)
                        .with_title(WindowTitle::text("Station Iapetus"))
                        .with_content(
                            GridBuilder::new(
                                WidgetBuilder::new()
                                    .with_margin(Thickness::uniform(20.0))
                                    .with_child({
                                        btn_new_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(0)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text("New Game")
                                        .with_font(font.clone())
                                        .build(ctx);
                                        btn_new_game
                                    })
                                    .with_child({
                                        btn_save_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(1)
                                                .with_enabled(false)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text("Save Game")
                                        .with_font(font.clone())
                                        .build(ctx);
                                        btn_save_game
                                    })
                                    .with_child({
                                        btn_load_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(2)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text("Load Game")
                                        .with_font(font.clone())
                                        .build(ctx);
                                        btn_load_game
                                    })
                                    .with_child({
                                        btn_settings = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(3)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text("Settings")
                                        .with_font(font.clone())
                                        .build(ctx);
                                        btn_settings
                                    })
                                    .with_child({
                                        btn_quit_game = ButtonBuilder::new(
                                            WidgetBuilder::new()
                                                .on_column(0)
                                                .on_row(4)
                                                .with_margin(Thickness::uniform(4.0)),
                                        )
                                        .with_text("Quit")
                                        .with_font(font)
                                        .build(ctx);
                                        btn_quit_game
                                    }),
                            )
                            .add_column(Column::stretch())
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .add_row(Row::strict(75.0))
                            .build(ctx),
                        )
                        .build(ctx),
                ),
        )
        .add_row(Row::stretch())
        .add_row(Row::strict(500.0))
        .add_column(Column::strict(400.0))
        .add_column(Column::stretch())
        .build(ctx);

        Self {
            scene,
            sender: sender.clone(),
            root,
            btn_new_game,
            btn_settings,
            btn_save_game,
            btn_load_game,
            btn_quit_game,
            options_menu: OptionsMenu::new(engine, control_scheme, sender),
        }
    }

    pub fn set_visible(&mut self, engine: &mut GameEngine, visible: bool) {
        engine.scenes[self.scene.scene].enabled = visible;

        engine
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.root,
                MessageDirection::ToWidget,
                visible,
            ));
        if !visible {
            engine.user_interface.send_message(WindowMessage::close(
                self.options_menu.window,
                MessageDirection::ToWidget,
            ));
        }
    }

    pub fn is_visible(&self, ui: &Gui) -> bool {
        ui.node(self.root).visibility()
    }

    pub fn process_input_event(
        &mut self,
        engine: &mut GameEngine,
        event: &Event<()>,
        control_scheme: &mut ControlScheme,
    ) {
        if let Event::WindowEvent {
            event: WindowEvent::Resized(new_size),
            ..
        } = event
        {
            engine.user_interface.send_message(WidgetMessage::width(
                self.root,
                MessageDirection::ToWidget,
                new_size.width as f32,
            ));
            engine.user_interface.send_message(WidgetMessage::height(
                self.root,
                MessageDirection::ToWidget,
                new_size.height as f32,
            ));
        }

        self.options_menu
            .process_input_event(engine, event, control_scheme);
    }

    pub fn sync_to_model(&mut self, engine: &mut GameEngine, level_loaded: bool) {
        engine.user_interface.send_message(WidgetMessage::enabled(
            self.btn_save_game,
            MessageDirection::ToWidget,
            level_loaded,
        ));
    }

    pub fn handle_ui_message(
        &mut self,
        engine: &mut GameEngine,
        level: Option<&Level>,
        message: &GuiMessage,
        control_scheme: &mut ControlScheme,
    ) {
        if let UiMessageData::Button(ButtonMessage::Click) = message.data() {
            if message.destination() == self.btn_new_game {
                self.sender.send(Message::StartNewGame).unwrap();
            } else if message.destination() == self.btn_save_game {
                self.sender.send(Message::SaveGame).unwrap();
            } else if message.destination() == self.btn_load_game {
                self.sender.send(Message::LoadGame).unwrap();
            } else if message.destination() == self.btn_quit_game {
                self.sender.send(Message::QuitGame).unwrap();
            } else if message.destination() == self.btn_settings {
                let is_visible = engine
                    .user_interface
                    .node(self.options_menu.window)
                    .visibility();

                if is_visible {
                    engine.user_interface.send_message(WindowMessage::close(
                        self.options_menu.window,
                        MessageDirection::ToWidget,
                    ));
                } else {
                    engine.user_interface.send_message(WindowMessage::open(
                        self.options_menu.window,
                        MessageDirection::ToWidget,
                        true,
                    ));
                }
            }
        }

        self.options_menu
            .handle_ui_event(engine, level, message, control_scheme);
    }
}
