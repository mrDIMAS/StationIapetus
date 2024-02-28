use crate::elevator::{ui::CallButtonUi, Elevator};
use fyrox::graph::{BaseSceneGraph, SceneGraph};
use fyrox::{
    asset::{manager::ResourceManager, Resource},
    core::{
        log::Log, pool::Handle, reflect::prelude::*, sstorage::ImmutableString, stub_uuid_provider,
        type_traits::prelude::*, variable::InheritableVariable, visitor::prelude::*,
    },
    engine::GraphicsContext,
    gui::UserInterface,
    material::{Material, MaterialResource, PropertyValue},
    resource::texture::{Texture, TextureResource},
    scene::{graph::Graph, mesh::Mesh, node::Node},
    script::{ScriptContext, ScriptTrait},
};
use strum_macros::{AsRefStr, EnumString, VariantNames};

#[derive(Debug, Visit, Reflect, Clone, AsRefStr, EnumString, VariantNames)]
pub enum CallButtonKind {
    FloorSelector,
    EndPoint,
}

stub_uuid_provider!(CallButtonKind);

impl Default for CallButtonKind {
    fn default() -> Self {
        Self::EndPoint
    }
}

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "215c9f84-a775-4d17-88a0-0e174c06dc4a")]
#[visit(optional)]
pub struct CallButton {
    pub floor: u32,
    pub kind: CallButtonKind,
    pub elevator: Handle<Node>,
    pub ui_resource: InheritableVariable<Option<Resource<UserInterface>>>,
    #[reflect(hidden)]
    #[visit(skip)]
    pub ui: Option<CallButtonUi>,
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
                        value: Some(resource_manager.request::<Texture>("data/ui/white_pixel.bmp")),
                        fallback: Default::default(),
                    },
                ));

                if let Some(first_surface) = mesh.surfaces_mut().get_mut(0) {
                    first_surface
                        .set_material(MaterialResource::new_ok(Default::default(), material));
                }
            }
        }
    }
}

impl ScriptTrait for CallButton {
    fn on_start(&mut self, context: &mut ScriptContext) {
        if let Some(ui_resource) = self.ui_resource.as_ref() {
            let ui = CallButtonUi::new(ui_resource.data_ref().clone(), self.floor);
            self.apply_screen_texture(
                context.handle,
                &mut context.scene.graph,
                context.resource_manager.clone(),
                ui.render_target.clone(),
            );
            self.ui = Some(ui);
        }
    }

    fn on_update(&mut self, context: &mut ScriptContext) {
        if let Some(ui) = self.ui.as_mut() {
            ui.update(context.dt);

            if let GraphicsContext::Initialized(graphics_context) = context.graphics_context {
                ui.render(&mut graphics_context.renderer);
            }
        }

        if let Some(elevator) = context
            .scene
            .graph
            .try_get(self.elevator)
            .and_then(|n| n.try_get_script::<Elevator>())
        {
            if let Some(ui) = self.ui.as_mut() {
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
}
