use crate::{character::character_ref, inventory::Inventory, Game, Level};
use fyrox::{
    animation::machine::{Event, Parameter},
    asset::manager::ResourceManager,
    core::{
        algebra::Vector3,
        log::Log,
        pool::Handle,
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    material::{Material, PropertyValue, SharedMaterial},
    resource::{
        model::ModelResource,
        texture::{Texture, TextureResource},
    },
    scene::{
        animation::absm::AnimationBlendingStateMachine,
        graph::Graph,
        mesh::Mesh,
        node::{Node, NodeHandle},
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};

pub mod ui;

#[derive(Debug, Clone, Default)]
struct OpenRequest {
    open: bool,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Door {
    #[reflect(description = "An array of handles to meshes that represents interactive screens.")]
    screens: Vec<NodeHandle>,

    #[visit(optional)]
    key_item: InheritableVariable<Option<ModelResource>>,

    #[visit(optional)]
    locked: InheritableVariable<bool>,

    #[visit(optional)]
    opened_state: InheritableVariable<String>,

    #[visit(optional)]
    opening_state: InheritableVariable<String>,

    #[visit(optional)]
    closed_state: InheritableVariable<String>,

    #[visit(optional)]
    closing_state: InheritableVariable<String>,

    #[visit(optional)]
    locked_state: InheritableVariable<String>,

    #[reflect(hidden)]
    #[visit(skip)]
    initial_position: Vector3<f32>,

    #[visit(optional)]
    state_machine: Handle<Node>,

    #[reflect(hidden)]
    #[visit(skip)]
    open_request: Option<OpenRequest>,

    #[reflect(hidden)]
    #[visit(skip)]
    self_handle: Handle<Node>,
}

impl Default for Door {
    fn default() -> Self {
        Self {
            screens: Default::default(),
            key_item: Default::default(),
            locked: Default::default(),
            opened_state: "Opened".to_string().into(),
            opening_state: "Open".to_string().into(),
            closed_state: "Closed".to_string().into(),
            closing_state: "Close".to_string().into(),
            locked_state: "Locked".to_string().into(),
            initial_position: Default::default(),
            state_machine: Default::default(),
            open_request: None,
            self_handle: Default::default(),
        }
    }
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

        Level::try_get_mut(ctx.plugins)
            .expect("Level must exist!")
            .doors_container
            .doors
            .push(ctx.handle);
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.self_handle = ctx.handle;

        self.initial_position = ctx.scene.graph[ctx.handle].global_position();

        let game = Game::game_mut(ctx.plugins);
        let texture = game.door_ui_container.create_ui(
            game.smaller_font.clone(),
            game.even_smaller_font.clone(),
            ctx.resource_manager.clone(),
            ctx.handle,
        );
        self.apply_screen_texture(&mut ctx.scene.graph, ctx.resource_manager.clone(), texture);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        // Level can not exist in case if we're changing the level. In this case there is no need
        // to unregister doors anyway, because the registry is already removed.
        if let Some(level) = Level::try_get_mut(ctx.plugins) {
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
        let game = Game::game_mut(ctx.plugins);
        let level = game.level.as_ref().unwrap();

        let mut closest_actor = None;
        let someone_nearby = level.actors.iter().any(|a| {
            let actor_position = character_ref(*a, &ctx.scene.graph).position(&ctx.scene.graph);
            let close_enough = actor_position.metric_distance(&self.initial_position) < 1.25;
            if close_enough {
                closest_actor = Some(a);
            }
            close_enough
        });

        let position = self.actual_position(&ctx.scene.graph);

        if let Some(state_machine) = ctx
            .scene
            .graph
            .try_get_mut_of_type::<AnimationBlendingStateMachine>(self.state_machine)
        {
            let open_request = self.open_request.take();

            let machine = state_machine.machine_mut().get_value_mut_silent();
            machine
                .set_parameter("Locked", Parameter::Rule(*self.locked))
                .set_parameter("SomeoneNearby", Parameter::Rule(someone_nearby))
                .set_parameter(
                    "Open",
                    Parameter::Rule(open_request.as_ref().map_or(false, |r| r.open)),
                );

            let mut sound = None;

            if let Some(layer) = machine.layers_mut().first_mut() {
                while let Some(event) = layer.pop_event() {
                    if let Event::ActiveStateChanged { new, .. } = event {
                        let new_state_name = layer.state(new).name.as_str();

                        if new_state_name == self.opening_state.as_str() {
                            sound = Some("data/sounds/door_open.ogg");
                        } else if new_state_name == self.closing_state.as_str() {
                            sound = Some("data/sounds/door_close.ogg");
                        }
                    }
                }

                if let Some(current_state) = layer.states().try_borrow(layer.active_state()) {
                    let text;
                    if current_state.name == self.opening_state.as_str() {
                        text = "Opening...";
                    } else if current_state.name == self.opened_state.as_str() {
                        text = "Opened";
                    } else if current_state.name == self.closing_state.as_str() {
                        text = "Closing..";
                    } else if current_state.name == self.closed_state.as_str() {
                        text = if someone_nearby { "Open?" } else { "Closed" };
                    } else if current_state.name == self.opened_state.as_str() {
                        text = "Locked";

                        if let Some(open_request) = open_request.as_ref() {
                            sound = Some(if open_request.open {
                                "data/sounds/access_granted.ogg"
                            } else {
                                "data/sounds/door_deny.ogg"
                            });
                        }
                    } else {
                        text = "Unknown";
                    };

                    if let Some(ui) = game.door_ui_container.get_ui_mut(ctx.handle) {
                        ui.update_text(text.to_owned(), &game.control_scheme);
                    }
                }
            }

            if let Some(sound) = sound {
                level.sound_manager.play_sound(
                    &mut ctx.scene.graph,
                    sound,
                    position,
                    0.6,
                    1.0,
                    1.0,
                );
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

impl Door {
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
        texture: TextureResource,
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
                        value: Some(
                            resource_manager.request::<Texture, _>("data/ui/white_pixel.bmp"),
                        ),
                        fallback: Default::default(),
                    },
                ));

                if let Some(first_surface) = mesh.surfaces_mut().get_mut(0) {
                    first_surface.set_material(SharedMaterial::new(material));
                }
            }
        }
    }

    pub fn try_open(&mut self, inventory: Option<&Inventory>) {
        let mut open = false;

        if let Some(inventory) = inventory {
            if let Some(key_item) = self.key_item.as_ref() {
                if inventory.item_count(key_item) > 0 {
                    open = true;
                }
            } else {
                open = true;
            }
        }

        self.open_request = Some(OpenRequest { open });
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
