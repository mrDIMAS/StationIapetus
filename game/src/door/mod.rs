use crate::{
    current_level_mut, game_mut, inventory::Inventory, item::ItemKind, message::Message, Actor,
    MessageSender,
};
use fyrox::{
    core::{
        algebra::Vector3,
        color::Color,
        inspect::prelude::*,
        parking_lot::Mutex,
        pool::Handle,
        reflect::Reflect,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        variable::{InheritableVariable, TemplateVariable},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider, impl_directly_inheritable_entity_trait,
    material::{Material, PropertyValue},
    resource::texture::Texture,
    scene::{
        graph::{map::NodeHandleMap, Graph},
        light::BaseLight,
        mesh::Mesh,
        node::{Node, NodeHandle, TypeUuidProvider},
        rigidbody::RigidBody,
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
    utils::log::Log,
};
use std::{
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::Arc,
};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

use crate::GameConstructor;

pub mod ui;

#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Reflect,
    Inspect,
    Visit,
    Debug,
    AsRefStr,
    EnumString,
    EnumVariantNames,
)]
#[repr(u32)]
pub enum DoorState {
    Opened = 0,
    Opening = 1,
    Closed = 2,
    Closing = 3,
    Locked = 4,
    Broken = 5,
}

impl Default for DoorState {
    fn default() -> Self {
        Self::Closed
    }
}

#[derive(
    Copy,
    Clone,
    Reflect,
    Inspect,
    Visit,
    Debug,
    AsRefStr,
    PartialEq,
    Eq,
    EnumString,
    EnumVariantNames,
)]
#[repr(C)]
pub enum DoorDirection {
    Side,
    Up,
}

impl Default for DoorDirection {
    fn default() -> Self {
        Self::Side
    }
}

#[derive(Debug, Clone, Default)]
struct OpenRequest {
    has_key: bool,
}

#[derive(Visit, Reflect, Inspect, Default, Debug, Clone)]
pub struct Door {
    #[inspect(
        description = "An array of handles to light sources that indicates state of the door."
    )]
    lights: Vec<NodeHandle>,

    #[inspect(description = "An array of handles to meshes that represents interactive screens.")]
    screens: Vec<NodeHandle>,

    #[inspect(
        deref,
        description = "A fixed direction of door to open. Given in local coordinates of the door.",
        is_modified = "is_modified()"
    )]
    #[visit(optional)]
    #[reflect(deref)]
    open_direction: TemplateVariable<DoorDirection>,

    #[inspect(
        deref,
        description = "Maximum offset along open_direction axis.",
        min_value = "0.0",
        is_modified = "is_modified()"
    )]
    #[visit(optional)]
    #[reflect(deref)]
    open_offset_amount: TemplateVariable<f32>,

    #[inspect(skip)]
    #[reflect(hidden)]
    #[visit(skip)]
    offset: f32,

    #[inspect(skip)]
    #[reflect(hidden)]
    #[visit(skip)]
    state: DoorState,

    #[inspect(skip)]
    #[reflect(hidden)]
    #[visit(skip)]
    initial_position: Vector3<f32>,

    #[inspect(skip)]
    #[reflect(hidden)]
    #[visit(skip)]
    open_request: Option<OpenRequest>,

    #[inspect(skip)]
    #[reflect(hidden)]
    #[visit(skip)]
    self_handle: Handle<Node>,
}

impl_component_provider!(Door);
impl_directly_inheritable_entity_trait!(Door; open_offset_amount);

impl TypeUuidProvider for Door {
    fn type_uuid() -> Uuid {
        uuid!("4b8aa92a-fe10-47d6-91bf-2878b834ff18")
    }
}

impl ScriptTrait for Door {
    fn on_init(&mut self, context: ScriptContext) {
        self.self_handle = context.handle;
        self.initial_position = context.scene.graph[context.handle].global_position();

        let game = game_mut(context.plugin);
        let texture = game.door_ui_container.create_ui(
            game.smaller_font.clone(),
            context.resource_manager.clone(),
            context.handle,
        );
        self.apply_screen_texture(
            &mut context.scene.graph,
            context.resource_manager.clone(),
            texture,
        );

        current_level_mut(context.plugin)
            .doors
            .doors
            .push(context.handle);
    }

    fn on_deinit(&mut self, context: ScriptDeinitContext) {
        let doors = &mut current_level_mut(context.plugin).doors.doors;
        if let Some(position) = doors.iter().position(|d| *d == context.node_handle) {
            doors.remove(position);
        }
    }

    fn on_update(&mut self, context: ScriptContext) {
        let ScriptContext {
            dt,
            plugin,
            handle,
            scene,
            ..
        } = context;

        let game = game_mut(plugin);

        let speed = 0.55;

        let node = &scene.graph[handle];
        let move_direction = match *self.open_direction {
            DoorDirection::Side => node.look_vector(),
            DoorDirection::Up => node.up_vector(),
        };

        let mut closest_actor = None;

        let someone_nearby = game.level.as_ref().map_or(false, |level| {
            level.actors.iter().any(|a| {
                let actor_position = a.position(&scene.graph);
                let close_enough = actor_position.metric_distance(&self.initial_position) < 1.25;
                if close_enough {
                    closest_actor = Some(a);
                }
                close_enough
            })
        });

        if !someone_nearby && self.state == DoorState::Opened {
            self.state = DoorState::Closing;

            game.message_sender.send(Message::PlaySound {
                path: PathBuf::from("data/sounds/door_close.ogg"),
                position: node.global_position(),
                gain: 0.6,
                rolloff_factor: 1.0,
                radius: 1.0,
            });
        }

        if let Some(ui) = game.door_ui_container.get_ui_mut(handle) {
            let text = match self.state {
                DoorState::Opened => "Opened",
                DoorState::Opening => "Opening...",
                DoorState::Closed => {
                    if someone_nearby {
                        "Open?"
                    } else {
                        "Closed"
                    }
                }
                DoorState::Closing => "Closing..",
                DoorState::Locked => "Locked",
                DoorState::Broken => "Broken",
            };

            ui.set_text(text.to_owned());
        }

        match self.state {
            DoorState::Opening => {
                if self.offset < *self.open_offset_amount {
                    self.offset += speed * dt;
                    if self.offset >= *self.open_offset_amount {
                        self.state = DoorState::Opened;
                        self.offset = *self.open_offset_amount;
                    }
                }

                self.set_lights_enabled(&mut scene.graph, false);
            }
            DoorState::Closing => {
                if self.offset > 0.0 {
                    self.offset -= speed * dt;
                    if self.offset <= 0.0 {
                        self.state = DoorState::Closed;
                        self.offset = 0.0;
                    }
                }

                self.set_lights_enabled(&mut scene.graph, false);
            }
            DoorState::Closed => {
                self.set_lights_enabled(&mut scene.graph, true);
                self.set_lights_color(&mut scene.graph, Color::opaque(0, 200, 0));
            }
            DoorState::Locked => {
                self.set_lights_enabled(&mut scene.graph, true);
                self.set_lights_color(&mut scene.graph, Color::opaque(200, 0, 0));
            }
            DoorState::Broken | DoorState::Opened => {
                self.set_lights_enabled(&mut scene.graph, false);
            }
        };

        if let Some(body) = scene.graph[context.handle].cast_mut::<RigidBody>() {
            body.local_transform_mut().set_position(
                self.initial_position
                    + move_direction
                        .try_normalize(f32::EPSILON)
                        .unwrap_or_default()
                        .scale(self.offset),
            );
        }

        if let Some(open_request) = self.open_request.take() {
            let position = self.actual_position(&scene.graph);

            if self.state == DoorState::Closed {
                self.state = DoorState::Opening;

                game.message_sender.send(Message::PlaySound {
                    path: PathBuf::from("data/sounds/door_open.ogg"),
                    position,
                    gain: 0.6,
                    rolloff_factor: 1.0,
                    radius: 1.0,
                });
            } else if self.state == DoorState::Locked {
                if open_request.has_key {
                    self.state = DoorState::Opening;

                    game.message_sender.send(Message::PlaySound {
                        path: PathBuf::from("data/sounds/door_open.ogg"),
                        position,
                        gain: 0.6,
                        rolloff_factor: 1.0,
                        radius: 1.0,
                    });

                    game.message_sender.send(Message::PlaySound {
                        path: PathBuf::from("data/sounds/access_granted.ogg"),
                        position,
                        gain: 1.0,
                        rolloff_factor: 1.0,
                        radius: 1.0,
                    });
                } else {
                    game.message_sender.send(Message::PlaySound {
                        path: PathBuf::from("data/sounds/door_deny.ogg"),
                        position,
                        gain: 1.0,
                        rolloff_factor: 1.0,
                        radius: 1.0,
                    });
                }
            }
        }
    }

    fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping.try_map_slice(&mut self.lights);
        old_new_mapping.try_map_slice(&mut self.screens);
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }

    fn plugin_uuid(&self) -> Uuid {
        GameConstructor::type_uuid()
    }
}

impl Door {
    fn set_lights_color(&self, graph: &mut Graph, color: Color) {
        for &light in self.lights.iter() {
            if let Some(light_ref) = graph[*light].query_component_mut::<BaseLight>() {
                light_ref.set_color(color);
            }
        }
    }

    fn set_lights_enabled(&self, graph: &mut Graph, enabled: bool) {
        for &light in self.lights.iter() {
            graph[*light].set_visibility(enabled);
        }
    }

    pub fn initial_position(&self) -> Vector3<f32> {
        self.initial_position
    }

    pub fn actual_position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.self_handle].global_position()
    }

    fn apply_screen_texture(
        &self,
        graph: &mut Graph,
        resource_manager: ResourceManager,
        texture: Texture,
    ) {
        for &node_handle in &self.screens {
            if let Some(mesh) = graph[*node_handle].cast_mut::<Mesh>() {
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
                        value: Some(resource_manager.request_texture("data/ui/white_pixel.bmp")),
                        fallback: Default::default(),
                    },
                ));

                if let Some(first_surface) = mesh.surfaces_mut().get_mut(0) {
                    first_surface.set_material(Arc::new(Mutex::new(material)));
                }
            }
        }
    }

    pub fn try_open(&mut self, inventory: Option<&Inventory>) {
        let has_key = inventory
            .map(|i| i.item_count(ItemKind::MasterKey) > 0)
            .unwrap_or(false);
        self.open_request = Some(OpenRequest { has_key });
    }
}

#[derive(Default, Visit)]
pub struct DoorContainer {
    pub doors: Vec<Handle<Node>>,
}

pub fn door_ref(handle: Handle<Node>, graph: &Graph) -> &Door {
    graph[handle]
        .script()
        .and_then(|s| s.cast::<Door>())
        .unwrap()
}

pub fn door_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Door {
    graph[handle]
        .script_mut()
        .and_then(|s| s.cast_mut::<Door>())
        .unwrap()
}

impl DoorContainer {
    pub fn new() -> Self {
        Self {
            doors: Default::default(),
        }
    }

    pub fn check_actor(
        &self,
        actor_position: Vector3<f32>,
        actor_handle: Handle<Actor>,
        graph: &Graph,
        sender: &MessageSender,
    ) {
        for &door_handle in &self.doors {
            let door = door_ref(door_handle, graph);
            let close_enough = actor_position.metric_distance(&door.initial_position()) < 1.25;
            if close_enough {
                sender.send(Message::TryOpenDoor {
                    door: door_handle,
                    actor: actor_handle,
                });
            }
        }
    }
}
