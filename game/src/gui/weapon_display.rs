use crate::{level::item::ItemKind, player::Player, weapon::weapon_ref};
use fyrox::{
    core::{algebra::Vector2, color::Color, pool::Handle},
    engine::resource_manager::ResourceManager,
    gui::{
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        message::MessageDirection,
        text::{TextBuilder, TextMessage},
        ttf::SharedFont,
        widget::WidgetBuilder,
        UiNode, UserInterface, VerticalAlignment,
    },
    resource::texture::Texture,
    scene::graph::Graph,
    utils,
};
use std::path::Path;

pub struct WeaponDisplay {
    pub ui: UserInterface,
    pub render_target: Texture,
    ammo: Handle<UiNode>,
    grenades: Handle<UiNode>,
}

impl WeaponDisplay {
    pub const WIDTH: f32 = 120.0;
    pub const HEIGHT: f32 = 120.0;

    pub fn new(font: SharedFont, resource_manager: ResourceManager) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

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
                    .with_texture(utils::into_gui_texture(
                        resource_manager.request_texture(Path::new("data/ui/ammo_icon.png")),
                    ))
                    .build(&mut ui.build_ctx()),
                )
                .with_child({
                    ammo = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_foreground(Brush::Solid(Color::opaque(0, 162, 232)))
                            .on_row(0)
                            .on_column(1),
                    )
                    .with_font(font.clone())
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
                    .with_texture(utils::into_gui_texture(
                        resource_manager.request_texture(Path::new("data/ui/grenade.png")),
                    ))
                    .build(&mut ui.build_ctx()),
                )
                .with_child({
                    grenades = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_foreground(Brush::Solid(Color::opaque(0, 162, 232)))
                            .on_row(1)
                            .on_column(1),
                    )
                    .with_font(font)
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
        let ammo = if player.current_weapon().is_some() {
            let total_ammo = player.inventory().item_count(ItemKind::Ammo);
            total_ammo
                / weapon_ref(player.current_weapon(), graph)
                    .definition
                    .ammo_consumption_per_shot
        } else {
            0
        };
        self.ui.send_message(TextMessage::text(
            self.ammo,
            MessageDirection::ToWidget,
            format!("{}", ammo),
        ));

        let grenades = player.inventory().item_count(ItemKind::Grenade);
        self.ui.send_message(TextMessage::text(
            self.grenades,
            MessageDirection::ToWidget,
            format!("{}", grenades),
        ));
    }

    pub fn update(&mut self, delta: f32) {
        self.ui.update(
            Vector2::new(WeaponDisplay::WIDTH, WeaponDisplay::HEIGHT),
            delta,
        );

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}
