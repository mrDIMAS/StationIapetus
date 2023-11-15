use crate::{elevator::Elevator, Game};
use fyrox::{
    asset::manager::ResourceManager,
    core::{
        log::Log,
        pool::Handle,
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    material::{Material, MaterialResource, PropertyValue},
    resource::texture::{Texture, TextureResource},
    scene::{graph::Graph, mesh::Mesh, node::Node},
    script::{ScriptContext, ScriptTrait},
};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(Debug, Visit, Reflect, Clone, AsRefStr, EnumString, EnumVariantNames)]
pub enum CallButtonKind {
    FloorSelector,
    EndPoint,
}

impl Default for CallButtonKind {
    fn default() -> Self {
        Self::EndPoint
    }
}

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct CallButton {
    pub floor: u32,
    pub kind: CallButtonKind,
    pub elevator: Handle<Node>,
}

impl CallButton {
    pub fn apply_screen_texture(
        &self,
        self_handle: Handle<Node>,
        graph: &mut Graph,
        resource_manager: ResourceManager,
        texture: TextureResource,
    ) {
        let screens = graph
            .traverse_handle_iter(self_handle)
            .filter(|h| graph[*h].name().starts_with("Screen"))
            .collect::<Vec<_>>();

        for node_handle in screens {
            if let Some(ref mut mesh) = graph[node_handle].cast_mut::<Mesh>() {
                let mut material = Material::standard();

                Log::verify(material.set_property(
                    &ImmutableString::new("diffuseTexture"),
                    PropertyValue::Sampler {
                        value: Some(texture.clone()),
                        fallback: Default::default(),
                    },
                ));

                Log::verify(material.set_property(
                    &ImmutableString::new("emissionTexture"),
                    PropertyValue::Sampler {
                        value: Some(
                            resource_manager.request::<Texture, _>("data/ui/white_pixel.bmp"),
                        ),
                        fallback: Default::default(),
                    },
                ));

                if let Some(first_surface) = mesh.surfaces_mut().get_mut(0) {
                    first_surface.set_material(MaterialResource::new_ok(material));
                }
            }
        }
    }
}

impl_component_provider!(CallButton);

impl TypeUuidProvider for CallButton {
    fn type_uuid() -> Uuid {
        uuid!("215c9f84-a775-4d17-88a0-0e174c06dc4a")
    }
}

impl ScriptTrait for CallButton {
    fn on_start(&mut self, context: &mut ScriptContext) {
        let game = Game::game_mut(context.plugins);

        let texture = game.call_button_ui_container.create_ui(
            game.smaller_font.clone(),
            context.handle,
            self.floor,
        );

        self.apply_screen_texture(
            context.handle,
            &mut context.scene.graph,
            context.resource_manager.clone(),
            texture,
        );
    }

    fn on_update(&mut self, context: &mut ScriptContext) {
        let game = Game::game_mut(context.plugins);

        if let Some(elevator) = context
            .scene
            .graph
            .try_get(self.elevator)
            .and_then(|n| n.try_get_script::<Elevator>())
        {
            if let Some(ui) = game.call_button_ui_container.get_ui_mut(context.handle) {
                ui.set_text(
                    if self.floor == elevator.current_floor {
                        "Ready"
                    } else if elevator.k.abs() > f32::EPSILON {
                        "Called"
                    } else {
                        "Call?"
                    }
                    .to_string(),
                );

                ui.set_floor_text(format!("Floor {}", self.floor));
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
