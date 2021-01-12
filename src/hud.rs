use crate::{GameEngine, Gui, UINodeHandle};
use rg3d::{
    core::color::Color,
    event::{Event, WindowEvent},
    gui::{
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        message::{MessageDirection, TextMessage, WidgetMessage},
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        ttf::{Font, SharedFont},
        widget::WidgetBuilder,
        HorizontalAlignment, Orientation, Thickness, VerticalAlignment,
    },
    utils,
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct Hud {
    root: UINodeHandle,
    health: UINodeHandle,
    armor: UINodeHandle,
    ammo: UINodeHandle,
    died: UINodeHandle,
}

impl Hud {
    pub fn new(engine: &mut GameEngine) -> Self {
        let frame_size = engine.renderer.get_frame_size();
        let ctx = &mut engine.user_interface.build_ctx();
        let resource_manager = engine.resource_manager.clone();

        let font = Font::from_file(
            Path::new("data/ui/SquaresBold.ttf"),
            35.0,
            Font::default_char_set(),
        )
        .unwrap();
        let font = SharedFont(Arc::new(Mutex::new(font)));

        let health;
        let armor;
        let ammo;
        let died;
        let root = GridBuilder::new(
            WidgetBuilder::new()
                .with_width(frame_size.0 as f32)
                .with_height(frame_size.1 as f32)
                .with_visibility(false)
                .with_child(
                    ImageBuilder::new(
                        WidgetBuilder::new()
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_width(33.0)
                            .with_height(33.0)
                            .on_row(0)
                            .on_column(1),
                    )
                    .with_texture(utils::into_gui_texture(
                        resource_manager.request_texture(Path::new("data/ui/crosshair.tga")),
                    ))
                    .build(ctx),
                )
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .on_column(0)
                            .on_row(0)
                            .with_vertical_alignment(VerticalAlignment::Bottom)
                            .with_margin(Thickness {
                                left: 50.0,
                                top: 0.0,
                                right: 0.0,
                                bottom: 150.0,
                            }),
                    )
                    .add_column(Column::strict(75.0))
                    .add_column(Column::strict(75.0))
                    .add_column(Column::strict(75.0))
                    .add_row(Row::strict(33.0))
                    .build(ctx),
                )
                .with_child(
                    StackPanelBuilder::new(
                        WidgetBuilder::new()
                            .with_margin(Thickness::bottom(10.0))
                            .on_column(0)
                            .with_vertical_alignment(VerticalAlignment::Bottom)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .with_child(
                                ImageBuilder::new(
                                    WidgetBuilder::new().with_width(35.0).with_height(35.0),
                                )
                                .with_texture(utils::into_gui_texture(
                                    resource_manager
                                        .request_texture(Path::new("data/ui/health_icon.png")),
                                ))
                                .build(ctx),
                            )
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new().with_width(170.0).with_height(35.0),
                                )
                                .with_text("Health:")
                                .with_font(font.clone())
                                .build(ctx),
                            )
                            .with_child({
                                health = TextBuilder::new(
                                    WidgetBuilder::new()
                                        .with_foreground(Brush::Solid(Color::opaque(180, 14, 22)))
                                        .with_width(170.0)
                                        .with_height(35.0),
                                )
                                .with_text("100")
                                .with_font(font.clone())
                                .build(ctx);
                                health
                            }),
                    )
                    .with_orientation(Orientation::Horizontal)
                    .build(ctx),
                )
                .with_child(
                    StackPanelBuilder::new(
                        WidgetBuilder::new()
                            .with_margin(Thickness::bottom(10.0))
                            .on_column(1)
                            .with_vertical_alignment(VerticalAlignment::Bottom)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .with_child(
                                ImageBuilder::new(
                                    WidgetBuilder::new().with_width(35.0).with_height(35.0),
                                )
                                .with_texture(utils::into_gui_texture(
                                    resource_manager
                                        .request_texture(Path::new("data/ui/ammo_icon.png")),
                                ))
                                .build(ctx),
                            )
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new().with_width(170.0).with_height(35.0),
                                )
                                .with_font(font.clone())
                                .with_text("Ammo:")
                                .build(ctx),
                            )
                            .with_child({
                                ammo = TextBuilder::new(
                                    WidgetBuilder::new()
                                        .with_foreground(Brush::Solid(Color::opaque(79, 79, 255)))
                                        .with_width(170.0)
                                        .with_height(35.0),
                                )
                                .with_font(font.clone())
                                .with_text("40")
                                .build(ctx);
                                ammo
                            }),
                    )
                    .with_orientation(Orientation::Horizontal)
                    .build(ctx),
                )
                .with_child(
                    StackPanelBuilder::new(
                        WidgetBuilder::new()
                            .with_margin(Thickness::bottom(10.0))
                            .on_column(2)
                            .with_vertical_alignment(VerticalAlignment::Bottom)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .with_child(
                                ImageBuilder::new(
                                    WidgetBuilder::new().with_width(35.0).with_height(35.0),
                                )
                                .with_texture(utils::into_gui_texture(
                                    resource_manager
                                        .request_texture(Path::new("data/ui/shield_icon.png")),
                                ))
                                .build(ctx),
                            )
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new().with_width(170.0).with_height(35.0),
                                )
                                .with_font(font.clone())
                                .with_text("Armor:")
                                .build(ctx),
                            )
                            .with_child({
                                armor = TextBuilder::new(
                                    WidgetBuilder::new()
                                        .with_foreground(Brush::Solid(Color::opaque(255, 100, 26)))
                                        .with_width(170.0)
                                        .with_height(35.0),
                                )
                                .with_font(font.clone())
                                .with_text("100")
                                .build(ctx);
                                armor
                            }),
                    )
                    .with_orientation(Orientation::Horizontal)
                    .build(ctx),
                )
                .with_child({
                    died = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_visibility(false)
                            .on_row(0)
                            .on_column(1)
                            .with_foreground(Brush::Solid(Color::opaque(200, 0, 0)))
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_horizontal_alignment(HorizontalAlignment::Center),
                    )
                    .with_font(font)
                    .with_text("You Died")
                    .build(ctx);
                    died
                }),
        )
        .add_column(Column::stretch())
        .add_column(Column::stretch())
        .add_column(Column::stretch())
        .add_row(Row::stretch())
        .build(ctx);

        Self {
            root,
            health,
            armor,
            ammo,
            died,
        }
    }

    pub fn set_health(&mut self, ui: &mut Gui, health: f32) {
        ui.send_message(TextMessage::text(
            self.health,
            MessageDirection::ToWidget,
            format!("{}", health),
        ));
    }

    pub fn set_armor(&mut self, ui: &mut Gui, armor: f32) {
        ui.send_message(TextMessage::text(
            self.armor,
            MessageDirection::ToWidget,
            format!("{}", armor),
        ));
    }

    pub fn set_ammo(&mut self, ui: &mut Gui, ammo: u32) {
        ui.send_message(TextMessage::text(
            self.ammo,
            MessageDirection::ToWidget,
            format!("{}", ammo),
        ));
    }

    pub fn set_visible(&mut self, ui: &mut Gui, visible: bool) {
        ui.send_message(WidgetMessage::visibility(
            self.root,
            MessageDirection::ToWidget,
            visible,
        ));
    }

    pub fn set_is_died(&mut self, ui: &mut Gui, is_died: bool) {
        ui.send_message(WidgetMessage::visibility(
            self.died,
            MessageDirection::ToWidget,
            is_died,
        ));
    }

    pub fn process_event(&mut self, engine: &mut GameEngine, event: &Event<()>) {
        if let Event::WindowEvent { event, .. } = event {
            if let WindowEvent::Resized(new_size) = event {
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
        }
    }
}
