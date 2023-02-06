use crate::{
    bot::Bot, config::SoundConfig, door::DoorContainer, level::item::ItemContainer,
    sound::SoundManager, utils::use_hrtf, MessageSender,
};
use fyrox::{
    core::{math::PositionProvider, pool::Handle, visitor::prelude::*},
    engine::resource_manager::ResourceManager,
    plugin::PluginContext,
    scene::{self, node::Node, Scene},
};
use std::path::Path;

pub mod death_zone;
pub mod decal;
pub mod item;
pub mod spawn;
pub mod trigger;
pub mod turret;

#[derive(Default, Visit)]
pub struct Level {
    pub map_path: String,
    pub scene: Handle<Scene>,
    pub player: Handle<Node>,
    pub actors: Vec<Handle<Node>>,
    pub items: ItemContainer,
    pub doors_container: DoorContainer,
    pub elevators: Vec<Handle<Node>>,

    #[visit(skip)]
    pub sound_manager: SoundManager,
    #[visit(skip)]
    sender: Option<MessageSender>,
}

impl Level {
    pub const ARRIVAL_PATH: &'static str = "data/levels/loading_bay.rgs";
    pub const TESTBED_PATH: &'static str = "data/levels/testbed.rgs";
    pub const LAB_PATH: &'static str = "data/levels/lab.rgs";

    pub fn from_existing_scene(
        scene: &mut Scene,
        scene_handle: Handle<Scene>,
        sender: MessageSender,
        sound_config: SoundConfig,
        resource_manager: ResourceManager,
    ) -> Self {
        if sound_config.use_hrtf {
            use_hrtf(&mut scene.graph.sound_context)
        } else {
            scene
                .graph
                .sound_context
                .state()
                .set_renderer(fyrox::scene::sound::Renderer::Default);
        }

        scene
            .graph
            .update(Default::default(), 0.0, Default::default());

        Self {
            player: Default::default(),
            actors: Default::default(),
            items: Default::default(),
            scene: scene_handle,
            sender: Some(sender),
            sound_manager: SoundManager::new(scene, resource_manager),
            doors_container: Default::default(),
            map_path: Default::default(),
            elevators: Default::default(),
        }
    }

    pub async fn new(
        map: String,
        resource_manager: ResourceManager,
        sender: MessageSender,
        sound_config: SoundConfig, // Using copy, instead of reference because of async.
    ) -> (Self, Scene) {
        let mut scene = Scene::new();

        if sound_config.use_hrtf {
            use_hrtf(&mut scene.graph.sound_context)
        } else {
            scene
                .graph
                .sound_context
                .state()
                .set_renderer(fyrox::scene::sound::Renderer::Default);
        }

        let map_model = resource_manager
            .request_model(Path::new(&map))
            .await
            .unwrap();

        // Instantiate map
        map_model.instantiate(&mut scene);

        scene
            .graph
            .update(Default::default(), 0.0, Default::default());

        let level = Self {
            player: Default::default(),
            actors: Default::default(),
            items: Default::default(),
            scene: Handle::NONE, // Filled when scene will be moved to engine.
            sender: Some(sender),
            sound_manager: SoundManager::new(&mut scene, resource_manager),
            doors_container: Default::default(),
            map_path: map,
            elevators: Default::default(),
        };

        (level, scene)
    }

    pub fn destroy(&mut self, context: &mut PluginContext) {
        context.scenes.remove(self.scene);
    }

    pub fn get_player(&self) -> Handle<Node> {
        self.player
    }

    pub fn resolve(&mut self, ctx: &mut PluginContext, sender: MessageSender) {
        self.set_message_sender(sender);
        self.sound_manager =
            SoundManager::new(&mut ctx.scenes[self.scene], ctx.resource_manager.clone());
    }

    pub fn set_message_sender(&mut self, sender: MessageSender) {
        self.sender = Some(sender);
    }

    pub fn debug_draw(&self, context: &mut PluginContext) {
        let scene = &mut context.scenes[self.scene];

        let drawing_context = &mut scene.drawing_context;

        drawing_context.clear_lines();

        scene.graph.physics.draw(drawing_context);

        for navmesh in scene.navmeshes.iter() {
            for pt in navmesh.vertices() {
                for neighbour in pt.neighbours() {
                    drawing_context.add_line(scene::debug::Line {
                        begin: pt.position(),
                        end: navmesh.vertices()[*neighbour as usize].position(),
                        color: Default::default(),
                    });
                }
            }

            for actor in self.actors.iter() {
                if let Some(bot) = scene.graph[*actor].try_get_script::<Bot>() {
                    bot.debug_draw(drawing_context);
                }
            }
        }
    }
}
