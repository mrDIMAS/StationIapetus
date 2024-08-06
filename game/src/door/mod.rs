use crate::{
    character::try_get_character_ref, door::ui::DoorUi, inventory::Inventory, utils, Game,
};
use fyrox::{
    asset::{manager::ResourceManager, Resource},
    core::{
        algebra::Vector3, log::Log, pool::Handle, reflect::prelude::*, type_traits::prelude::*,
        variable::InheritableVariable, visitor::prelude::*,
    },
    engine::GraphicsContext,
    graph::SceneGraph,
    gui::UserInterface,
    material::{Material, MaterialResource, MaterialResourceExtension},
    resource::{
        model::ModelResource,
        texture::{Texture, TextureResource},
    },
    scene::{animation::absm::prelude::*, graph::Graph, mesh::Mesh, node::Node},
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};

pub mod ui;

#[derive(Debug, Clone, Default)]
struct OpenRequest {
    open: bool,
}

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "4b8aa92a-fe10-47d6-91bf-2878b834ff18")]
#[visit(optional)]
pub struct Door {
    #[reflect(description = "An array of handles to meshes that represents interactive screens.")]
    screens: InheritableVariable<Vec<Handle<Node>>>,
    open_sound: InheritableVariable<Handle<Node>>,
    close_sound: InheritableVariable<Handle<Node>>,
    access_granted_sound: InheritableVariable<Handle<Node>>,
    access_denied_sound: InheritableVariable<Handle<Node>>,
    key_item: InheritableVariable<Option<ModelResource>>,
    pub locked: InheritableVariable<bool>,
    opened_state: InheritableVariable<String>,
    opening_state: InheritableVariable<String>,
    closed_state: InheritableVariable<String>,
    closing_state: InheritableVariable<String>,
    locked_state: InheritableVariable<String>,
    ui_resource: InheritableVariable<Option<Resource<UserInterface>>>,

    #[visit(skip)]
    #[reflect(hidden)]
    ui: Option<DoorUi>,

    #[reflect(hidden)]
    #[visit(skip)]
    initial_position: Vector3<f32>,

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
            open_sound: Default::default(),
            close_sound: Default::default(),
            access_granted_sound: Default::default(),
            access_denied_sound: Default::default(),
            key_item: Default::default(),
            locked: Default::default(),
            opened_state: "Opened".to_string().into(),
            opening_state: "Open".to_string().into(),
            closed_state: "Closed".to_string().into(),
            closing_state: "Close".to_string().into(),
            locked_state: "Locked".to_string().into(),
            ui_resource: Default::default(),
            ui: Default::default(),
            initial_position: Default::default(),
            state_machine: Default::default(),
            open_request: None,
            self_handle: Default::default(),
        }
    }
}

impl ScriptTrait for Door {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        self.initial_position = ctx.scene.graph[ctx.handle].global_position();

        ctx.plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .expect("Level must exist!")
            .doors_container
            .doors
            .push(ctx.handle);
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.self_handle = ctx.handle;

        self.initial_position = ctx.scene.graph[ctx.handle].global_position();

        if let Some(ui_resource) = self.ui_resource.as_ref() {
            let ui = DoorUi::new(ui_resource.data_ref().clone());
            self.apply_screen_texture(
                &mut ctx.scene.graph,
                ctx.resource_manager.clone(),
                ui.render_target.clone(),
            );
            self.ui = Some(ui);
        }
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        // Level can not exist in case if we're changing the level. In this case there is no need
        // to unregister doors anyway, because the registry is already removed.
        if let Some(level) = ctx.plugins.get_mut::<Game>().level.as_mut() {
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
        if let Some(ui) = self.ui.as_mut() {
            ui.update(ctx.dt);
            if let GraphicsContext::Initialized(graphics_context) = ctx.graphics_context {
                ui.render(&mut graphics_context.renderer);
            }
        }

        let game = ctx.plugins.get_mut::<Game>();
        let level = game.level.as_ref().unwrap();

        let mut closest_actor = None;
        let someone_nearby = level.actors.iter().any(|a| {
            if let Some(actor) = try_get_character_ref(*a, &ctx.scene.graph) {
                let actor_position = actor.position(&ctx.scene.graph);
                let close_enough = actor_position.metric_distance(&self.initial_position) < 1.25;
                if close_enough {
                    closest_actor = Some(a);
                }
                close_enough
            } else {
                false
            }
        });

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

            let mut sound = Handle::NONE;

            if let Some(layer) = machine.layers_mut().first_mut() {
                while let Some(event) = layer.pop_event() {
                    if let Event::ActiveStateChanged { new, .. } = event {
                        let new_state_name = layer.state(new).name.as_str();

                        if new_state_name == self.opening_state.as_str() {
                            sound = *self.open_sound;
                        } else if new_state_name == self.closing_state.as_str() {
                            sound = *self.close_sound;
                        }
                    }
                }

                if let Some(current_state) = layer.states().try_borrow(layer.active_state()) {
                    let mut can_interact = false;
                    let mut locked = false;
                    let text;
                    if current_state.name == self.opening_state.as_str() {
                        text = "Opening...";
                    } else if current_state.name == self.opened_state.as_str() {
                        text = "Opened";
                    } else if current_state.name == self.closing_state.as_str() {
                        text = "Closing..";
                    } else if current_state.name == self.closed_state.as_str() {
                        if someone_nearby {
                            can_interact = true;
                            text = "Open?";
                        } else {
                            text = "Closed";
                        }
                    } else if current_state.name == self.locked_state.as_str() {
                        text = "Locked";
                        locked = true;

                        if let Some(open_request) = open_request.as_ref() {
                            sound = if open_request.open {
                                *self.access_granted_sound
                            } else {
                                *self.access_denied_sound
                            };
                        }
                    } else {
                        text = "Unknown";
                    };

                    if let Some(ui) = self.ui.as_mut() {
                        ui.update_text(
                            text.to_owned(),
                            &game.config.controls,
                            can_interact,
                            locked,
                        );
                    }
                }
            }

            utils::try_play_sound(sound, &mut ctx.scene.graph);
        }
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
        for &node_handle in self.screens.iter() {
            if let Some(mesh) = graph[node_handle].cast_mut::<Mesh>() {
                let mut material = Material::standard();

                Log::verify(material.set_property("diffuseTexture", texture.clone()));
                Log::verify(material.set_property(
                    "emissionTexture",
                    resource_manager.request::<Texture>("data/ui/white_pixel.bmp"),
                ));

                if let Some(first_surface) = mesh.surfaces_mut().get_mut(0) {
                    first_surface.set_material(MaterialResource::new(material));
                }
            }
        }
    }

    pub fn try_open(&mut self, inventory: Option<&Inventory>) {
        let mut open = false;

        if *self.locked {
            if let Some(inventory) = inventory {
                if let Some(key_item) = self.key_item.as_ref() {
                    if inventory.item_count(key_item) > 0 {
                        open = true;
                        self.locked.set_value_and_mark_modified(false);
                    }
                }
            }
        } else {
            open = true;
        }

        self.open_request = Some(OpenRequest { open });
    }
}

#[derive(Default, Visit, Debug)]
pub struct DoorContainer {
    pub doors: Vec<Handle<Node>>,
}

pub fn door_ref(handle: Handle<Node>, graph: &Graph) -> &Door {
    graph.try_get_script_of::<Door>(handle).unwrap()
}

pub fn door_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Door {
    graph.try_get_script_of_mut::<Door>(handle).unwrap()
}

impl DoorContainer {
    pub fn new() -> Self {
        Self {
            doors: Default::default(),
        }
    }
}
