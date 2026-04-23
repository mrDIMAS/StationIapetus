use crate::{
    config::{Config, SoundConfig},
    gui::{
        options_menu::OptionsMenu,
        save_load::{Mode, SaveLoadDialog},
    },
    message::Message,
    MessageSender,
};
use fyrox::gui::border::Border;
use fyrox::{
    asset::io::FsResourceIo,
    core::{pool::Handle, visitor::prelude::*},
    event::Event,
    graph::SceneGraph,
    gui::{
        border::BorderBuilder,
        button::{Button, ButtonBuilder, ButtonMessage},
        font::FontResource,
        message::UiMessage,
        screen::{Screen, ScreenBuilder},
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::{WidgetBuilder, WidgetMessage},
        window::WindowMessage,
        BuildContext, HorizontalAlignment, Thickness, UserInterface, VerticalAlignment,
    },
    plugin::PluginContext,
    scene::{sound::Sound, Scene, SceneLoader},
};

#[derive(Visit, Default, Debug)]
pub struct Menu {
    pub scene: MenuScene,
    root: Handle<Screen>,
    main_menu: Handle<Border>,
    btn_new_game: Handle<Button>,
    btn_save_game: Handle<Button>,
    btn_settings: Handle<Button>,
    btn_load_game: Handle<Button>,
    btn_quit_game: Handle<Button>,
    options_menu: Option<OptionsMenu>,
    save_load_dialog: Option<SaveLoadDialog>,
    font: FontResource,
}

#[derive(Visit, Default, Debug)]
pub struct MenuScene {
    pub scene: Handle<Scene>,
    pub music: Handle<Sound>,
}

impl MenuScene {
    pub async fn new(context: &mut PluginContext<'_, '_>, sound_config: &SoundConfig) -> Self {
        let mut scene = SceneLoader::from_file(
            "data/levels/menu.rgs",
            &FsResourceIo,
            context.serialization_context.clone(),
            context.dyn_type_constructors.clone(),
            context.resource_manager.clone(),
        )
        .await
        .unwrap()
        .0
        .finish()
        .await;

        let music = scene
            .graph
            .find_handle_by_name_from_root("Music")
            .to_variant::<Sound>();

        if let Ok(music) = scene.graph.try_get_mut(music) {
            music.set_gain(sound_config.music_volume);
        }

        Self {
            music,
            scene: context.scenes.add(scene),
        }
    }
}

fn make_button(text: &str, font: FontResource, ctx: &mut BuildContext) -> Handle<Button> {
    ButtonBuilder::new(
        WidgetBuilder::new()
            .with_height(75.0)
            .with_margin(Thickness::uniform(4.0)),
    )
    .with_content(
        TextBuilder::new(WidgetBuilder::new())
            .with_text(text)
            .with_font(font)
            .with_font_size(30.0.into())
            .with_vertical_text_alignment(VerticalAlignment::Center)
            .with_horizontal_text_alignment(HorizontalAlignment::Center)
            .build(ctx),
    )
    .build(ctx)
}

impl Menu {
    pub async fn new(
        context: &mut PluginContext<'_, '_>,
        font: FontResource,
        config: &Config,
    ) -> Self {
        let scene = MenuScene::new(context, &config.sound).await;

        let ctx = &mut context.user_interfaces.first_mut().build_ctx();

        let btn_new_game;
        let btn_settings;
        let btn_save_game;
        let btn_load_game;
        let btn_quit_game;
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(20.0))
                .with_child({
                    btn_new_game = make_button("New Game", font.clone(), ctx);
                    btn_new_game
                })
                .with_child({
                    btn_save_game = make_button("Save Game", font.clone(), ctx);
                    btn_save_game
                })
                .with_child({
                    btn_load_game = make_button("Load Game", font.clone(), ctx);
                    btn_load_game
                })
                .with_child({
                    btn_settings = make_button("Settings", font.clone(), ctx);
                    btn_settings
                })
                .with_child({
                    btn_quit_game = make_button("Quit", font.clone(), ctx);
                    btn_quit_game
                }),
        )
        .build(ctx);

        let main_menu = BorderBuilder::new(
            WidgetBuilder::new()
                .on_row(1)
                .on_column(0)
                .with_width(400.0)
                .with_height(500.0)
                .with_horizontal_alignment(HorizontalAlignment::Left)
                .with_vertical_alignment(VerticalAlignment::Center)
                .with_margin(Thickness::uniform(4.0))
                .with_child(content),
        )
        .with_corner_radius(4.0.into())
        .with_pad_by_corner_radius(false)
        .build(ctx);

        let root = ScreenBuilder::new(
            WidgetBuilder::new().with_child(main_menu).with_child(
                TextBuilder::new(
                    WidgetBuilder::new()
                        .with_horizontal_alignment(HorizontalAlignment::Center)
                        .with_vertical_alignment(VerticalAlignment::Top),
                )
                .with_font_size(60.0.into())
                .with_font(font.clone())
                .with_text("Station Iapetus")
                .build(ctx),
            ),
        )
        .build(ctx);

        Self {
            scene,
            root,
            main_menu,
            btn_new_game,
            btn_settings,
            btn_save_game,
            btn_load_game,
            btn_quit_game,
            options_menu: None,
            save_load_dialog: None,
            font,
        }
    }

    pub fn set_visible(&mut self, context: &mut PluginContext, visible: bool) {
        let ui = context.user_interfaces.first_mut();

        if let Ok(scene) = context.scenes.try_get_mut(self.scene.scene) {
            scene.enabled.set_value_silent(visible);
        }

        ui.send(self.root, WidgetMessage::Visibility(visible));
        if !visible {
            if let Some(options_menu) = self.options_menu.as_ref() {
                ui.send(options_menu.window, WindowMessage::Close);
            }
        }
    }

    pub fn is_visible(&self, ui: &UserInterface) -> bool {
        ui[self.root].visibility()
    }

    pub fn process_input_event(
        &mut self,
        ctx: &mut PluginContext,
        event: &Event<()>,
        config: &mut Config,
    ) {
        if let Some(options_menu) = self.options_menu.as_mut() {
            options_menu.process_input_event(ctx, event, config);
        }
    }

    pub fn sync_to_model(&mut self, ctx: &mut PluginContext, level_loaded: bool) {
        ctx.user_interfaces
            .first()
            .send(self.btn_save_game, WidgetMessage::Enabled(level_loaded));
    }

    fn on_save_game_clicked(&mut self, ui: &mut UserInterface) {
        self.save_load_dialog = Some(SaveLoadDialog::new(
            Mode::Save,
            self.font.clone(),
            &mut ui.build_ctx(),
        ));
    }

    fn on_load_game_clicked(&mut self, ui: &mut UserInterface) {
        self.save_load_dialog = Some(SaveLoadDialog::new(
            Mode::Load,
            self.font.clone(),
            &mut ui.build_ctx(),
        ));
    }

    fn on_settings_clicked(&mut self, ctx: &mut PluginContext, config: &mut Config) {
        if let Some(options_menu) = self.options_menu.as_ref() {
            ctx.user_interfaces
                .first()
                .send(options_menu.window, WindowMessage::Close);
        } else {
            self.options_menu = Some(OptionsMenu::new(ctx, config));
        }
    }

    pub fn handle_ui_message(
        &mut self,
        ctx: &mut PluginContext,
        message: &UiMessage,
        config: &mut Config,
        sender: &MessageSender,
    ) {
        if let Some(options_menu) = self.options_menu.take() {
            self.options_menu = options_menu.handle_ui_event(ctx, message, config, sender);
        }

        let ui = ctx.user_interfaces.first_mut();

        let no_opened_screens = self.options_menu.is_none() && self.save_load_dialog.is_none();
        ui.send_sync(self.main_menu, WidgetMessage::Visibility(no_opened_screens));

        if let Some(save_load_dialog) = self.save_load_dialog.take() {
            self.save_load_dialog = save_load_dialog.handle_ui_message(message, ui, sender);
        }

        if let Some(ButtonMessage::Click) = message.data_from(self.btn_new_game) {
            sender.send(Message::StartNewGame);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.btn_save_game) {
            self.on_save_game_clicked(ui);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.btn_load_game) {
            self.on_load_game_clicked(ui);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.btn_quit_game) {
            sender.send(Message::QuitGame);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.btn_settings) {
            self.on_settings_clicked(ctx, config)
        }
    }
}
