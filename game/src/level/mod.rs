use crate::{
    bot::Bot,
    character::{character_ref, try_get_character_mut, CharacterCommand},
    config::SoundConfig,
    door::DoorContainer,
    item::ItemContainer,
    message::Message,
    sound::{SoundKind, SoundManager},
    utils::use_hrtf,
    MessageSender,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        math::{ray::Ray, PositionProvider},
        pool::Handle,
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    plugin::PluginContext,
    scene::{self, graph::physics::RayCastOptions, node::Node, Scene},
};
use std::path::Path;

pub mod death_zone;
pub mod decal;
pub mod spawn;
pub mod trail;
pub mod trigger;
pub mod turret;

#[derive(Default, Visit)]
pub struct Level {
    pub map_path: String,
    pub scene: Handle<Scene>,
    pub player: Handle<Node>,
    pub actors: Vec<Handle<Node>>,
    pub items: ItemContainer,
    sound_manager: SoundManager,
    pub doors_container: DoorContainer,
    pub elevators: Vec<Handle<Node>>,

    #[visit(skip)]
    sender: Option<MessageSender>,
}

pub fn footstep_ray_check(
    begin: Vector3<f32>,
    scene: &mut Scene,
    self_collider: Handle<Node>,
    sender: MessageSender,
) {
    let mut query_buffer = Vec::new();

    let ray = Ray::from_two_points(begin, begin + Vector3::new(0.0, -100.0, 0.0));

    scene.graph.physics.cast_ray(
        RayCastOptions {
            ray_origin: Point3::from(ray.origin),
            ray_direction: ray.dir,
            max_len: 100.0,
            groups: Default::default(),
            sort_results: true,
        },
        &mut query_buffer,
    );

    for intersection in query_buffer
        .into_iter()
        .filter(|i| i.collider != self_collider)
    {
        sender.send(Message::PlayEnvironmentSound {
            collider: intersection.collider,
            feature: intersection.feature,
            position: intersection.position.coords,
            sound_kind: SoundKind::FootStep,
            gain: 0.2,
            rolloff_factor: 1.0,
            radius: 0.3,
        });
    }
}

impl Level {
    pub const ARRIVAL_PATH: &'static str = "data/levels/loading_bay.rgs";
    pub const TESTBED_PATH: &'static str = "data/levels/testbed.rgs";
    pub const LAB_PATH: &'static str = "data/levels/lab.rgs";

    pub fn from_existing_scene(
        scene: &mut Scene,
        scene_handle: Handle<Scene>,
        sender: MessageSender,
        sound_config: SoundConfig, // Using copy, instead of reference because of async.
    ) -> Self {
        if sound_config.use_hrtf {
            use_hrtf(&mut scene.graph.sound_context)
        } else {
            scene
                .graph
                .sound_context
                .set_renderer(fyrox::scene::sound::Renderer::Default);
        }

        scene.graph.update(Default::default(), 0.0);

        Self {
            player: Default::default(),
            actors: Default::default(),
            items: Default::default(),
            scene: scene_handle,
            sender: Some(sender),
            sound_manager: SoundManager::new(scene),
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
                .set_renderer(fyrox::scene::sound::Renderer::Default);
        }

        let map_model = resource_manager
            .request_model(Path::new(&map))
            .await
            .unwrap();

        // Instantiate map
        map_model.instantiate_geometry(&mut scene);

        scene.graph.update(Default::default(), 0.0);

        let level = Self {
            player: Default::default(),
            actors: Default::default(),
            items: Default::default(),
            scene: Handle::NONE, // Filled when scene will be moved to engine.
            sender: Some(sender),
            sound_manager: SoundManager::new(&mut scene),
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

    fn apply_splash_damage(
        &mut self,
        engine: &mut PluginContext,
        amount: f32,
        radius: f32,
        center: Vector3<f32>,
        who: Handle<Node>,
        critical_shot_probability: f32,
    ) {
        let scene = &mut engine.scenes[self.scene];
        // Just find out actors which must be damaged and re-cast damage message for each.
        for &actor_handle in self.actors.iter() {
            let character = character_ref(actor_handle, &scene.graph);
            // TODO: Add occlusion test. This will hit actors through walls.
            let position = character.position(&scene.graph);
            if position.metric_distance(&center) <= radius {
                if let Some(character) = try_get_character_mut(actor_handle, &mut scene.graph) {
                    character.push_command(CharacterCommand::Damage {
                        who,
                        hitbox: None,
                        /// TODO: Maybe collect all hitboxes?
                        amount,
                        critical_shot_probability,
                    });
                }
            }
        }
    }

    pub async fn handle_message(&mut self, engine: &mut PluginContext<'_>, message: &Message) {
        self.sound_manager
            .handle_message(
                &mut engine.scenes[self.scene].graph,
                engine.resource_manager.clone(),
                message,
            )
            .await;

        match message {
            &Message::ApplySplashDamage {
                amount,
                radius,
                center,
                who,
                critical_shot_probability,
            } => self.apply_splash_damage(
                engine,
                amount,
                radius,
                center,
                who,
                critical_shot_probability,
            ),
            _ => (),
        }
    }

    pub fn resolve(&mut self, engine: &mut PluginContext, sender: MessageSender) {
        self.set_message_sender(sender);
        let scene = &mut engine.scenes[self.scene];
        self.sound_manager.resolve(scene);
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
