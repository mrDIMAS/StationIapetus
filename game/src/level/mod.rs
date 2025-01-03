use crate::{
    bot::Bot, config::SoundConfig, door::DoorContainer, level::item::ItemContainer,
    sound::SoundManager, utils::use_hrtf, MessageSender,
};
use fyrox::fxhash::FxHashSet;
use fyrox::graph::SceneGraph;
use fyrox::{
    asset::manager::ResourceManager,
    core::{futures::executor::block_on, pool::Handle, visitor::prelude::*},
    plugin::PluginContext,
    scene::{
        navmesh::NavigationalMesh,
        node::{Node, NodeTrait},
        Scene,
    },
};

pub mod arrival;
pub mod death_zone;
pub mod decal;
pub mod explosion;
pub mod explosive_barrel;
pub mod hit_box;
pub mod item;
pub mod point_of_interest;
pub mod spawn;
pub mod trigger;
pub mod turret;

#[derive(Default, Visit, Debug)]
pub struct Level {
    pub scene: Handle<Scene>,
    pub player: Handle<Node>,
    pub actors: Vec<Handle<Node>>,
    pub death_zones: FxHashSet<Handle<Node>>,
    pub hit_boxes: FxHashSet<Handle<Node>>,
    pub items: ItemContainer,
    pub doors_container: DoorContainer,
    pub elevators: Vec<Handle<Node>>,
    pub navmesh: Handle<Node>,
    pub pois: FxHashSet<Handle<Node>>,

    #[visit(skip)]
    pub sound_manager: SoundManager,
    #[visit(skip)]
    sender: Option<MessageSender>,
}

impl Level {
    //pub const ARRIVAL_PATH: &'static str = "data/levels/arrival.rgs";
    pub const ARRIVAL_PATH: &'static str = "data/levels/testbed.rgs";

    pub fn from_existing_scene(
        scene: &mut Scene,
        scene_handle: Handle<Scene>,
        sender: MessageSender,
        sound_config: SoundConfig,
        resource_manager: ResourceManager,
    ) -> Self {
        if sound_config.use_hrtf {
            block_on(use_hrtf(&mut scene.graph.sound_context, &resource_manager))
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

        let navmesh = scene
            .graph
            .find_from_root(&mut |n| n.cast::<NavigationalMesh>().is_some())
            .map(|t| t.0)
            .unwrap_or_default();

        Self {
            navmesh,
            player: Default::default(),
            actors: Default::default(),
            death_zones: Default::default(),
            hit_boxes: Default::default(),
            items: Default::default(),
            scene: scene_handle,
            sender: Some(sender),
            sound_manager: SoundManager::new(scene, resource_manager),
            doors_container: Default::default(),
            elevators: Default::default(),
            pois: Default::default(),
        }
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

        if let Some(navmesh) = scene
            .graph
            .try_get_of_type::<NavigationalMesh>(self.navmesh)
        {
            navmesh.debug_draw(drawing_context);
        }

        for actor in self.actors.iter() {
            if let Some(bot) = scene.graph[*actor].try_get_script::<Bot>() {
                bot.debug_draw(drawing_context);
            }
        }
    }
}
