use crate::player::Player;
use crate::weapon::Weapon;
use fyrox::resource::texture::{TextureResource, TextureResourceExtension};
use fyrox::{
    asset::manager::ResourceManager,
    core::{algebra::Vector2, color::Color, pool::Handle},
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
    pub render_target: TextureResource,
    ammo: Handle<UiNode>,
    grenades: Handle<UiNode>,
}

impl WeaponDisplay {
    pub const WIDTH: f32 = 120.0;
    pub const HEIGHT: f32 = 120.0;

    pub fn new(font: SharedFont, resource_manager: ResourceManager) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target =
            TextureResource::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

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
                        resource_manager.request::<Texture, _>(Path::new("data/ui/ammo_icon.png")),
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
                        resource_manager.request::<Texture, _>(Path::new("data/ui/grenade.png")),
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
        let ammo = if let Some(weapon) = graph.try_get_script_of::<Weapon>(player.current_weapon())
        {
            if let Some(ammo_item) = weapon.ammo_item.as_ref() {
                let total_ammo = player.inventory().item_count(ammo_item);
                total_ammo / *weapon.ammo_consumption_per_shot
            } else {
                0
            }
        } else {
            0
        };
        self.ui.send_message(TextMessage::text(
            self.ammo,
            MessageDirection::ToWidget,
            format!("{ammo}"),
        ));

        if let Some(grenade_item) = player.grenade_item.as_ref() {
            let grenades = player.inventory().item_count(grenade_item);
            self.ui.send_message(TextMessage::text(
                self.grenades,
                MessageDirection::ToWidget,
                format!("{grenades}"),
            ));
        }
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
