use crate::{
    config::{Config, SoundConfig},
    gui::{
        options_menu::OptionsMenu,
        save_load::{Mode, SaveLoadDialog},
    },
    message::Message,
    Game, MessageSender,
};
use fyrox::{
    core::{err, pool::Handle, reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*},
    event::Event,
    graph::SceneGraph,
    gui::{
        button::{Button, ButtonMessage},
        font::FontResource,
        grid::Grid,
        message::UiMessage,
        widget::WidgetMessage,
        window::WindowMessage,
        UserInterface,
    },
    plugin::{error::GameResult, PluginContext},
    scene::{sound::Sound, Scene},
};

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider)]
#[type_uuid(id = "0226e2fe-799c-4ad9-9e57-6aec808b5af1")]
#[visit(optional)]
pub struct MenuData {
    btn_new_game: Handle<Button>,
    btn_save_game: Handle<Button>,
    btn_settings: Handle<Button>,
    btn_load_game: Handle<Button>,
    btn_quit_game: Handle<Button>,
    container: Handle<Grid>,
}

#[derive(Visit, Default, Debug)]
pub struct Menu {
    pub scene: Option<MenuScene>,
    ui: Handle<UserInterface>,
    data: MenuData,
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
    pub fn new(
        mut scene: Scene,
        context: &mut PluginContext<'_, '_>,
        sound_config: &SoundConfig,
    ) -> Self {
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

impl Menu {
    pub fn new(
        mut user_interface: UserInterface,
        context: &mut PluginContext<'_, '_>,
        font: FontResource,
    ) -> Self {
        context.load_scene(
            "data/levels/menu.rgs",
            false,
            |result, game: &mut Game, ctx| {
                if let Some(menu) = game.menu.as_mut() {
                    menu.scene = Some(MenuScene::new(result?.payload, ctx, &game.config.sound));
                }
                Ok(())
            },
        );

        let data = user_interface
            .user_data
            .try_take::<MenuData>()
            .ok()
            .unwrap_or_else(|| {
                err!("There's no menu data in the main menu ui! Fallback to default.");
                MenuData::default()
            });
        let ui = context.user_interfaces.add(user_interface);

        Self {
            scene: None,
            ui,
            data,
            options_menu: None,
            save_load_dialog: None,
            font,
        }
    }

    pub fn set_visible(&mut self, context: &mut PluginContext, visible: bool) -> GameResult {
        let ui = context.user_interfaces.try_get_mut(self.ui)?;

        if let Some(scene) = self
            .scene
            .as_ref()
            .and_then(|s| context.scenes.try_get_mut(s.scene).ok())
        {
            scene.enabled.set_value_silent(visible);
        }

        ui.send(ui.root(), WidgetMessage::Visibility(visible));
        if !visible {
            if let Some(options_menu) = self.options_menu.as_ref() {
                ui.send(options_menu.window, WindowMessage::Close);
            }
        }

        Ok(())
    }

    pub fn is_visible(&self, ctx: &PluginContext) -> bool {
        ctx.user_interfaces
            .try_get(self.ui)
            .ok()
            .is_some_and(|ui| ui[ui.root()].visibility())
    }

    pub fn process_input_event(
        &mut self,
        ctx: &mut PluginContext,
        event: &Event<()>,
        config: &mut Config,
    ) -> GameResult {
        if let Some(options_menu) = self.options_menu.as_mut() {
            options_menu.process_input_event(ctx, event, config)?;
        }
        Ok(())
    }

    pub fn sync_to_model(&mut self, ctx: &mut PluginContext, level_loaded: bool) -> GameResult {
        let ui = ctx.user_interfaces.try_get(self.ui)?;
        ui.send(
            self.data.btn_save_game,
            WidgetMessage::Enabled(level_loaded),
        );
        Ok(())
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

    fn on_settings_clicked(&mut self, ctx: &mut PluginContext, config: &mut Config) -> GameResult {
        if let Some(options_menu) = self.options_menu.as_ref() {
            let ui = ctx.user_interfaces.try_get(self.ui)?;
            ui.send(options_menu.window, WindowMessage::Close);
        } else {
            self.options_menu = Some(OptionsMenu::new(self.ui, ctx, config));
        }
        Ok(())
    }

    pub fn update(&self, ctx: &mut PluginContext) -> GameResult {
        let ui = ctx.user_interfaces.try_get(self.ui)?;

        let no_opened_screens = self.options_menu.is_none() && self.save_load_dialog.is_none();
        ui.send_sync(
            self.data.container,
            WidgetMessage::Visibility(no_opened_screens),
        );

        Ok(())
    }

    pub fn handle_ui_message(
        &mut self,
        ctx: &mut PluginContext,
        ui_handle: Handle<UserInterface>,
        message: &UiMessage,
        config: &mut Config,
        sender: &MessageSender,
    ) -> GameResult {
        if self.ui != ui_handle {
            return Ok(());
        }

        if let Some(options_menu) = self.options_menu.take() {
            self.options_menu = options_menu.handle_ui_event(ctx, message, config, sender)?;
        }

        let ui = ctx.user_interfaces.try_get_mut(self.ui)?;

        if let Some(save_load_dialog) = self.save_load_dialog.take() {
            self.save_load_dialog = save_load_dialog.handle_ui_message(message, ui, sender);
        }

        if let Some(ButtonMessage::Click) = message.data_from(self.data.btn_new_game) {
            sender.send(Message::StartNewGame);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.data.btn_save_game) {
            self.on_save_game_clicked(ui);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.data.btn_load_game) {
            self.on_load_game_clicked(ui);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.data.btn_quit_game) {
            sender.send(Message::QuitGame);
        } else if let Some(ButtonMessage::Click) = message.data_from(self.data.btn_settings) {
            self.on_settings_clicked(ctx, config)?;
        }

        Ok(())
    }
}
