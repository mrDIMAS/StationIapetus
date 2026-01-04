use crate::{gui, player::Player, weapon::Weapon};
use fyrox::{
    asset::manager::ResourceManager,
    core::{algebra::Vector2, color::Color, pool::Handle, visitor::prelude::*},
    gui::{
        brush::Brush,
        font::FontResource,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        text::{TextBuilder, TextMessage},
        widget::WidgetBuilder,
        UiNode, UserInterface, VerticalAlignment,
    },
    resource::texture::{Texture, TextureResource},
    scene::graph::Graph,
};
use std::path::Path;

#[derive(Visit, Default, Debug)]
pub struct WeaponDisplay {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    ammo: Handle<UiNode>,
    grenades: Handle<UiNode>,
}

impl WeaponDisplay {
    pub const WIDTH: f32 = 120.0;
    pub const HEIGHT: f32 = 120.0;

    pub fn new(font: FontResource, resource_manager: ResourceManager) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT);

        let ammo;
        let grenades;
        GridBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    ImageBuilder::new(
                        WidgetBuilder::new()
                            .with_width(32.0)
                            .with_height(32.0)
                            .on_row(0)
                            .on_column(0),
                    )
                    .with_texture(
                        resource_manager.request::<Texture>(Path::new("data/ui/ammo_icon.png")),
                    )
                    .build(&mut ui.build_ctx()),
                )
                .with_child({
                    ammo = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_foreground(Brush::Solid(Color::opaque(0, 162, 232)).into())
                            .on_row(0)
                            .on_column(1),
                    )
                    .with_font(font.clone())
                    .with_font_size(31.0.into())
                    .build(&mut ui.build_ctx());
                    ammo
                })
                .with_child(
                    ImageBuilder::new(
                        WidgetBuilder::new()
                            .with_width(32.0)
                            .with_height(32.0)
                            .on_row(1)
                            .on_column(0),
                    )
                    .with_texture(
                        resource_manager.request::<Texture>(Path::new("data/ui/grenade.png")),
                    )
                    .build(&mut ui.build_ctx()),
                )
                .with_child({
                    grenades = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_foreground(Brush::Solid(Color::opaque(0, 162, 232)).into())
                            .on_row(1)
                            .on_column(1),
                    )
                    .with_font(font)
                    .with_font_size(31.0.into())
                    .build(&mut ui.build_ctx());
                    grenades
                }),
        )
        .add_column(Column::auto())
        .add_column(Column::stretch())
        .add_row(Row::auto())
        .add_row(Row::auto())
        .add_row(Row::stretch())
        .build(&mut ui.build_ctx());

        Self {
            ui,
            render_target,
            ammo,
            grenades,
        }
    }

    pub fn sync_to_model(&self, player: &Player, graph: &Graph) {
        let ammo = if let Ok(weapon) =
            graph.try_get_script_component_of::<Weapon>(player.current_weapon())
        {
            if let Some(ammo_item) = weapon.ammo_item.as_ref() {
                let total_ammo = player.inventory().item_count(ammo_item);
                total_ammo / *weapon.ammo_consumption_per_shot
            } else {
                u32::MAX
            }
        } else {
            0
        };

        self.ui.send(
            self.ammo,
            TextMessage::Text(if ammo == u32::MAX {
                "INF".to_string()
            } else {
                format!("{ammo}")
            }),
        );

        if let Some(grenade_item) = player.grenade_item.as_ref() {
            let grenades = player.inventory().item_count(grenade_item);
            self.ui
                .send(self.grenades, TextMessage::Text(format!("{grenades}")));
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.ui.update(
            Vector2::new(WeaponDisplay::WIDTH, WeaponDisplay::HEIGHT),
            delta,
            &Default::default(),
        );

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}
