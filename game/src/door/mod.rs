use crate::{character::character_ref, current_level_mut, game_mut};
use fyrox::{
    core::{
        algebra::Vector3,
        color::Color,
        pool::Handle,
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    material::{Material, PropertyValue, SharedMaterial},
    resource::texture::Texture,
    scene::{
        graph::Graph,
        light::BaseLight,
        mesh::Mesh,
        node::{Node, NodeHandle, TypeUuidProvider},
        rigidbody::RigidBody,
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
    utils::log::Log,
};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

pub mod ui;

#[derive(
    Copy, Clone, Eq, PartialEq, Reflect, Visit, Debug, AsRefStr, EnumString, EnumVariantNames,
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
    Copy, Clone, Reflect, Visit, Debug, AsRefStr, PartialEq, Eq, EnumString, EnumVariantNames,
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

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct Door {
    #[reflect(
        description = "An array of handles to light sources that indicates state of the door."
    )]
    lights: Vec<NodeHandle>,

    #[reflect(description = "An array of handles to meshes that represents interactive screens.")]
    screens: Vec<NodeHandle>,

    #[reflect(
        description = "A fixed direction of door to open. Given in local coordinates of the door."
    )]
    #[visit(optional)]
    open_direction: InheritableVariable<DoorDirection>,

    #[reflect(
        description = "Maximum offset along open_direction axis.",
        min_value = "0.0"
    )]
    #[visit(optional)]
    open_offset_amount: InheritableVariable<f32>,

    #[reflect(hidden)]
    #[visit(skip)]
    offset: f32,

    #[reflect(hidden)]
    #[visit(skip)]
    state: DoorState,

    #[reflect(hidden)]
    initial_position: Vector3<f32>,

    #[reflect(hidden)]
    #[visit(skip)]
    open_request: Option<OpenRequest>,

    #[reflect(hidden)]
    #[visit(skip)]
    self_handle: Handle<Node>,
}

impl_component_provider!(Door);

impl TypeUuidProvider for Door {
    fn type_uuid() -> Uuid {
        uuid!("4b8aa92a-fe10-47d6-91bf-2878b834ff18")
    }
}

impl ScriptTrait for Door {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        self.initial_position = ctx.scene.graph[ctx.handle].global_position();

        current_level_mut(ctx.plugins)
            .expect("Level must exist!")
            .doors_container
            .doors
            .push(ctx.handle);
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.self_handle = ctx.handle;

        let game = game_mut(ctx.plugins);
        let texture = game.door_ui_container.create_ui(
            game.smaller_font.clone(),
            ctx.resource_manager.clone(),
            ctx.handle,
        );
        self.apply_screen_texture(&mut ctx.scene.graph, ctx.resource_manager.clone(), texture);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        // Level can not exist in case if we're changing the level. In this case there is no need
        // to unregister doors anyway, because the registry is already removed.
        if let Some(level) = current_level_mut(ctx.plugins) {
            if let Some(position) = level
                .doors_container
                .doors
                .iter()
                .position(|d| *d == ctx.node_handle)
            {
                level.doors_container.doors.remove(position);
            }
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = game_mut(ctx.plugins);
        let level = game.level.as_ref().unwrap();

        let speed = 0.55;

        let node = &ctx.scene.graph[ctx.handle];
        let move_direction = match *self.open_direction {
            DoorDirection::Side => node.look_vector(),
            DoorDirection::Up => node.up_vector(),
        };

        let mut closest_actor = None;

        let someone_nearby = level.actors.iter().any(|a| {
            let actor_position = character_ref(*a, &ctx.scene.graph).position(&ctx.scene.graph);
            let close_enough = actor_position.metric_distance(&self.initial_position) < 1.25;
            if close_enough {
                closest_actor = Some(a);
            }
            close_enough
        });

        if !someone_nearby && self.state == DoorState::Opened {
            self.state = DoorState::Closing;
            let position = node.global_position();
            level.sound_manager.play_sound(
                &mut ctx.scene.graph,
                "data/sounds/door_close.ogg",
                position,
                0.6,
                1.0,
                1.0,
            );
        }

        if let Some(ui) = game.door_ui_container.get_ui_mut(ctx.handle) {
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
                    self.offset += speed * ctx.dt;
                    if self.offset >= *self.open_offset_amount {
                        self.state = DoorState::Opened;
                        self.offset = *self.open_offset_amount;
                    }
                }

                self.set_lights_enabled(&mut ctx.scene.graph, false);
            }
            DoorState::Closing => {
                if self.offset > 0.0 {
                    self.offset -= speed * ctx.dt;
                    if self.offset <= 0.0 {
                        self.state = DoorState::Closed;
                        self.offset = 0.0;
                    }
                }

                self.set_lights_enabled(&mut ctx.scene.graph, false);
            }
            DoorState::Closed => {
                self.set_lights_enabled(&mut ctx.scene.graph, true);
                self.set_lights_color(&mut ctx.scene.graph, Color::opaque(0, 200, 0));
            }
            DoorState::Locked => {
                self.set_lights_enabled(&mut ctx.scene.graph, true);
                self.set_lights_color(&mut ctx.scene.graph, Color::opaque(200, 0, 0));
            }
            DoorState::Broken | DoorState::Opened => {
                self.set_lights_enabled(&mut ctx.scene.graph, false);
            }
        };

        if let Some(body) = ctx.scene.graph[ctx.handle].cast_mut::<RigidBody>() {
            body.local_transform_mut().set_position(
                self.initial_position
                    + move_direction
                        .try_normalize(f32::EPSILON)
                        .unwrap_or_default()
                        .scale(self.offset),
            );
        }

        if let Some(open_request) = self.open_request.take() {
            let position = self.actual_position(&ctx.scene.graph);

            if self.state == DoorState::Closed {
                self.state = DoorState::Opening;

                level.sound_manager.play_sound(
                    &mut ctx.scene.graph,
                    "data/sounds/door_open.ogg",
                    position,
                    0.6,
                    1.0,
                    1.0,
                );
            } else if self.state == DoorState::Locked {
                if open_request.has_key {
                    self.state = DoorState::Opening;

                    level.sound_manager.play_sound(
                        &mut ctx.scene.graph,
                        "data/sounds/door_open.ogg",
                        position,
                        0.6,
                        1.0,
                        1.0,
                    );

                    level.sound_manager.play_sound(
                        &mut ctx.scene.graph,
                        "data/sounds/access_granted.ogg",
                        position,
                        1.0,
                        1.0,
                        1.0,
                    );
                } else {
                    level.sound_manager.play_sound(
                        &mut ctx.scene.graph,
                        "data/sounds/door_deny.ogg",
                        position,
                        1.0,
                        1.0,
                        1.0,
                    );
                }
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
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
                    first_surface.set_material(SharedMaterial::new(material));
                }
            }
        }
    }

    pub fn try_open(&mut self, has_key: bool) {
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
}
