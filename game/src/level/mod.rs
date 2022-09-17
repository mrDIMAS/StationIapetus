use crate::{
    actor::{Actor, ActorContainer},
    bot::{Bot, BotKind},
    character::HitBox,
    config::SoundConfig,
    door::{door_mut, DoorContainer},
    effects::{self, EffectKind},
    elevator::{
        call_button::{CallButton, CallButtonContainer, CallButtonKind},
        Elevator, ElevatorContainer,
    },
    item::ItemContainer,
    level::{
        decal::Decal,
        trail::{ShotTrail, ShotTrailContainer},
        trigger::{Trigger, TriggerContainer, TriggerKind},
    },
    light::{Light, LightContainer},
    message::Message,
    player::{Player, PlayerPersistentData},
    sound::{SoundKind, SoundManager},
    utils::{is_probability_event_occurred, use_hrtf},
    weapon::weapon_mut,
    weapon::{
        definition::{ShotEffect, WeaponKind},
        projectile::{Damage, Projectile, ProjectileContainer, ProjectileKind, Shooter},
        ray_hit,
        sight::SightReaction,
    },
    CallButtonUiContainer, GameTime, MessageSender,
};
use fyrox::{
    core::{
        algebra::{Point3, UnitQuaternion, Vector3},
        color::Color,
        futures::executor::block_on,
        math::{aabb::AxisAlignedBoundingBox, ray::Ray, vector_to_quat, PositionProvider},
        parking_lot::Mutex,
        pool::Handle,
        rand::seq::SliceRandom,
        sstorage::ImmutableString,
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    material::{Material, PropertyValue},
    plugin::PluginContext,
    rand,
    resource::texture::Texture,
    scene::{
        self, base,
        base::BaseBuilder,
        collider::ColliderShape,
        graph::physics::RayCastOptions,
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder, RenderPath,
        },
        node::Node,
        rigidbody::RigidBody,
        transform::TransformBuilder,
        Scene,
    },
    utils::{log::Log, navmesh::Navmesh},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

pub mod decal;
pub mod trail;
pub mod trigger;
pub mod turret;

#[derive(Default, Visit)]
pub struct Level {
    pub map_path: String,
    pub scene: Handle<Scene>,
    pub player: Handle<Node>,
    pub projectiles: ProjectileContainer,
    pub actors: ActorContainer,
    pub items: ItemContainer,
    spawn_points: Vec<SpawnPoint>,
    #[visit(skip)]
    sender: Option<MessageSender>,
    pub navmesh: Handle<Navmesh>,
    death_zones: Vec<DeathZone>,
    time: f32,
    sound_manager: SoundManager,
    #[visit(skip)]
    beam: Option<Arc<Mutex<SurfaceData>>>,
    trails: ShotTrailContainer,
    pub doors_container: DoorContainer,
    lights: LightContainer,
    triggers: TriggerContainer,
    pub elevators: ElevatorContainer,
    pub call_buttons: CallButtonContainer,
}

#[derive(Visit, Default)]
pub struct DeathZone {
    bounds: AxisAlignedBoundingBox,
}

pub struct UpdateContext<'a> {
    pub time: GameTime,
    pub scene: &'a mut Scene,
    pub items: &'a ItemContainer,
    pub doors: &'a DoorContainer,
    pub navmesh: Handle<Navmesh>,
    pub sender: &'a MessageSender,
    pub elevators: &'a ElevatorContainer,
    pub call_buttons: &'a CallButtonContainer,
}

#[derive(Default)]
pub struct AnalysisResult {
    death_zones: Vec<DeathZone>,
    spawn_points: Vec<SpawnPoint>,
    player_spawn_position: Vector3<f32>,
    player_spawn_orientation: UnitQuaternion<f32>,
    doors: DoorContainer,
    lights: LightContainer,
    triggers: TriggerContainer,
    elevators: ElevatorContainer,
    call_buttons: CallButtonContainer,
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

fn make_beam() -> Arc<Mutex<SurfaceData>> {
    Arc::new(Mutex::new(SurfaceData::make_cylinder(
        6,
        1.0,
        1.0,
        false,
        &UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 90.0f32.to_radians()).to_homogeneous(),
    )))
}

pub async fn analyze(scene: &mut Scene) -> AnalysisResult {
    let mut result = AnalysisResult::default();

    let mut spawn_points = Vec::new();
    let mut death_zones = Vec::new();
    let mut player_spawn_position = Default::default();
    let mut player_spawn_orientation = Default::default();
    let mut triggers = TriggerContainer::default();
    let mut elevators = ElevatorContainer::new();
    let mut call_buttons = CallButtonContainer::new();

    for (handle, node) in scene.graph.pair_iter() {
        let name = node.name();

        if name.starts_with("Zombie") {
            spawn_points.push(SpawnPoint {
                position: node.global_position(),
                rotation: **node.local_transform().rotation(),
                bot_kind: BotKind::Zombie,
                spawned: false,
                with_gun: false,
            })
        } else if name.starts_with("Mutant") {
            spawn_points.push(SpawnPoint {
                position: node.global_position(),
                rotation: **node.local_transform().rotation(),
                bot_kind: BotKind::Mutant,
                with_gun: false,
                spawned: false,
            })
        } else if name.starts_with("Parasite") {
            spawn_points.push(SpawnPoint {
                position: node.global_position(),
                rotation: **node.local_transform().rotation(),
                bot_kind: BotKind::Parasite,
                spawned: false,
                with_gun: false,
            })
        } else if name.starts_with("PlayerSpawnPoint") {
            player_spawn_position = node.global_position();
            player_spawn_orientation = scene.graph.global_rotation(handle);
        } else if name.starts_with("DeathZone") && node.is_mesh() {
            death_zones.push(handle);
        }

        if node.tag().starts_with("Elevator") {
            let elevator = elevators.add(Elevator::new(handle));
            let elevator_mut = &mut elevators[elevator];

            for property in node.properties.iter() {
                if let base::PropertyValue::NodeHandle(node_handle) = property.value {
                    if let Some(node_ref) = scene.graph.try_get(node_handle) {
                        if property.name == "PathPoint" {
                            elevator_mut.points.push(node_ref.global_position());
                        } else if property.name == "CallButton" {
                            if let Some(base::PropertyValue::U32(floor)) =
                                node_ref.find_first_property_ref("Floor").map(|p| &p.value)
                            {
                                let call_button = call_buttons.add(CallButton::new(
                                    elevator,
                                    node_handle,
                                    *floor,
                                    CallButtonKind::EndPoint,
                                ));

                                elevator_mut.call_buttons.push(call_button);
                            } else {
                                Log::err("Call button is missing Floor parameter!")
                            }
                        } else if property.name == "FloorSelector" {
                            let call_button = call_buttons.add(CallButton::new(
                                elevator,
                                node_handle,
                                0,
                                CallButtonKind::FloorSelector,
                            ));

                            elevator_mut.call_buttons.push(call_button);
                        }
                    }
                }
            }
        }

        match node.tag() {
            "PlayerSpawnPoint" => {
                player_spawn_position = node.global_position();
                player_spawn_orientation = scene.graph.global_rotation(handle);
            }
            "FlashingLight" => result.lights.add(Light::new(handle)),
            "NextLevelTrigger" => triggers.add(Trigger::new(handle, TriggerKind::NextLevel)),
            "EndGameTrigger" => triggers.add(Trigger::new(handle, TriggerKind::EndGame)),
            "ZombieWithGun" => spawn_points.push(SpawnPoint {
                position: node.global_position(),
                rotation: **node.local_transform().rotation(),
                bot_kind: BotKind::Zombie,
                spawned: false,
                with_gun: true,
            }),
            _ => (),
        }
    }

    for handle in death_zones {
        let node = &mut scene.graph[handle];
        node.set_visibility(false);
        result.death_zones.push(DeathZone {
            bounds: node.as_mesh().world_bounding_box(),
        });
    }
    result.spawn_points = spawn_points;
    result.player_spawn_position = player_spawn_position;
    result.player_spawn_orientation = player_spawn_orientation;
    result.triggers = triggers;
    result.elevators = elevators;
    result.call_buttons = call_buttons;

    result
}

async fn spawn_player(
    spawn_position: Vector3<f32>,
    orientation: UnitQuaternion<f32>,
    resource_manager: ResourceManager,
    scene: &mut Scene,
    persistent_data: Option<PlayerPersistentData>,
) -> Handle<Node> {
    let player = Player::add_to_scene(scene, resource_manager.clone()).await;

    scene.graph[player]
        .local_transform_mut()
        .set_position(spawn_position)
        .set_rotation(orientation);

    player
}

async fn spawn_bot(
    spawn_point: &mut SpawnPoint,
    actors: &mut ActorContainer,
    resource_manager: ResourceManager,
    scene: &mut Scene,
    weapon: Option<WeaponKind>,
) -> Handle<Actor> {
    spawn_point.spawned = true;

    let bot = add_bot(
        spawn_point.bot_kind,
        spawn_point.position,
        spawn_point.rotation,
        actors,
        resource_manager.clone(),
        scene,
    )
    .await;

    bot
}

async fn add_bot(
    kind: BotKind,
    position: Vector3<f32>,
    rotation: UnitQuaternion<f32>,
    actors: &mut ActorContainer,
    resource_manager: ResourceManager,
    scene: &mut Scene,
) -> Handle<Actor> {
    let bot = Bot::new(kind, resource_manager.clone(), scene, position, rotation).await;
    actors.add(Actor::Bot(bot))
}

impl Level {
    pub const ARRIVAL_PATH: &'static str = "data/levels/loading_bay.rgs";
    pub const TESTBED_PATH: &'static str = "data/levels/testbed.rgs";
    pub const LAB_PATH: &'static str = "data/levels/lab.rgs";

    pub fn from_existing_scene(
        scene: &mut Scene,
        scene_handle: Handle<Scene>,
        resource_manager: ResourceManager,
        sender: MessageSender,
        sound_config: SoundConfig, // Using copy, instead of reference because of async.
        persistent_data: Option<PlayerPersistentData>,
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

        let AnalysisResult {
            death_zones,
            mut spawn_points,
            player_spawn_position,
            player_spawn_orientation,
            doors,
            lights,
            triggers,
            elevators,
            call_buttons,
        } = block_on(analyze(scene));
        let mut actors = ActorContainer::new();

        for pt in spawn_points.iter_mut() {
            block_on(spawn_bot(
                pt,
                &mut actors,
                resource_manager.clone(),
                scene,
                if pt.with_gun {
                    Some(WeaponKind::Ak47)
                } else {
                    None
                },
            ));
        }

        Self {
            player: block_on(spawn_player(
                player_spawn_position,
                player_spawn_orientation,
                resource_manager,
                scene,
                persistent_data,
            )),
            actors,
            items: Default::default(),
            lights,
            death_zones,
            spawn_points,
            triggers,
            navmesh: scene.navmeshes.handle_from_index(0),
            scene: scene_handle,
            sender: Some(sender),
            time: 0.0,
            projectiles: ProjectileContainer::new(),
            sound_manager: SoundManager::new(scene),
            beam: Some(make_beam()),
            trails: Default::default(),
            doors_container: doors,
            elevators,
            call_buttons,
            map_path: Default::default(),
        }
    }

    pub async fn new(
        map: String,
        resource_manager: ResourceManager,
        sender: MessageSender,
        sound_config: SoundConfig, // Using copy, instead of reference because of async.
        persistent_data: Option<PlayerPersistentData>,
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

        let AnalysisResult {
            death_zones,
            mut spawn_points,
            player_spawn_position,
            player_spawn_orientation,
            doors,
            lights,
            triggers,
            elevators,
            call_buttons,
        } = analyze(&mut scene).await;
        let mut actors = ActorContainer::new();

        for pt in spawn_points.iter_mut() {
            spawn_bot(
                pt,
                &mut actors,
                resource_manager.clone(),
                &mut scene,
                if pt.with_gun {
                    Some(WeaponKind::Ak47)
                } else {
                    None
                },
            )
            .await;
        }

        let level = Self {
            player: spawn_player(
                player_spawn_position,
                player_spawn_orientation,
                resource_manager.clone(),
                &mut scene,
                persistent_data,
            )
            .await,
            actors,
            items: Default::default(),
            lights,
            death_zones,
            spawn_points,
            triggers,
            navmesh: scene.navmeshes.handle_from_index(0),
            scene: Handle::NONE, // Filled when scene will be moved to engine.
            sender: Some(sender),
            time: 0.0,
            projectiles: ProjectileContainer::new(),
            sound_manager: SoundManager::new(&mut scene),
            beam: Some(make_beam()),
            trails: Default::default(),
            doors_container: doors,
            elevators,
            call_buttons,
            map_path: map,
        };

        (level, scene)
    }

    pub fn destroy(&mut self, context: &mut PluginContext) {
        context.scenes.remove(self.scene);
    }

    pub fn get_player(&self) -> Handle<Node> {
        self.player
    }

    pub fn actors(&self) -> &ActorContainer {
        &self.actors
    }

    pub fn actors_mut(&mut self) -> &mut ActorContainer {
        &mut self.actors
    }

    async fn add_bot(
        &mut self,
        engine: &mut PluginContext<'_>,
        kind: BotKind,
        position: Vector3<f32>,
        rotation: UnitQuaternion<f32>,
    ) -> Handle<Actor> {
        add_bot(
            kind,
            position,
            rotation,
            &mut self.actors,
            engine.resource_manager.clone(),
            &mut engine.scenes[self.scene],
        )
        .await
    }

    async fn remove_actor(&mut self, engine: &mut PluginContext<'_>, actor: Handle<Actor>) {
        if self.actors.contains(actor) {
            let scene = &mut engine.scenes[self.scene];
            self.actors.get_mut(actor).clean_up(scene);
            self.actors.free(actor);
        }
    }

    async fn create_projectile(
        &mut self,
        engine: &mut PluginContext<'_>,
        kind: ProjectileKind,
        position: Vector3<f32>,
        direction: Vector3<f32>,
        initial_velocity: Vector3<f32>,
        owner: Shooter,
    ) {
        let scene = &mut engine.scenes[self.scene];
        let projectile = Projectile::new(
            kind,
            engine.resource_manager.clone(),
            scene,
            direction,
            position,
            owner,
            initial_velocity,
        )
        .await;
        self.projectiles.add(projectile);
    }

    fn damage_actor(
        &mut self,
        engine: &mut PluginContext,
        actor_handle: Handle<Actor>,
        who: Handle<Actor>,
        mut amount: f32,
        hitbox: Option<HitBox>,
        critical_shot_probability: f32,
    ) {
        if self.actors.contains(actor_handle)
            && (who.is_none() || who.is_some() && self.actors.contains(who))
        {
            let scene = &mut engine.scenes[self.scene];

            let who_position = if who.is_some() {
                Some(self.actors.get(who).position(&scene.graph))
            } else {
                None
            };
            let actor = self.actors.get_mut(actor_handle);

            if !actor.is_dead() {
                if let Actor::Bot(bot) = actor {
                    if let Some(who_position) = who_position {
                        bot.set_target(actor_handle, who_position);
                    }
                }

                if let Some(hitbox) = hitbox {
                    // Handle critical head shots.
                    let critical_head_shot_probability = critical_shot_probability.clamp(0.0, 1.0); // * 100.0%
                    if hitbox.is_head
                        && is_probability_event_occurred(critical_head_shot_probability)
                    {
                        amount *= 1000.0;

                        if let Actor::Bot(bot) = actor {
                            bot.blow_up_head(&mut scene.graph);
                        }
                    }
                }

                actor.damage(amount);

                // Prevent spamming with grunt sounds.
                if actor.last_health - actor.health > 20.0 {
                    actor.last_health = actor.health;
                    match actor {
                        Actor::Bot(bot) => {
                            bot.restoration_time = 0.8;

                            if let Some(grunt_sound) =
                                bot.definition.pain_sounds.choose(&mut rand::thread_rng())
                            {
                                self.sender.as_ref().unwrap().send(Message::PlaySound {
                                    path: PathBuf::from(grunt_sound.clone()),
                                    position: actor.position(&scene.graph),
                                    gain: 0.8,
                                    rolloff_factor: 1.0,
                                    radius: 0.6,
                                });
                            }
                        }
                        Actor::Player(_) => {
                            // TODO: Add player sounds.
                        }
                    }
                }
            }
        }
    }

    fn update_death_zones(&mut self, scene: &Scene) {
        for (handle, actor) in self.actors.pair_iter_mut() {
            for death_zone in self.death_zones.iter() {
                if death_zone
                    .bounds
                    .is_contains_point(actor.position(&scene.graph))
                {
                    self.sender.as_ref().unwrap().send(Message::DamageActor {
                        actor: handle,
                        who: Default::default(),
                        hitbox: None,
                        amount: 99999.0,
                        critical_shot_probability: 0.0,
                    });
                }
            }
        }
    }

    fn update_game_ending(&self, scene: &Scene) {
        let player_ref = scene.graph[self.player].try_get_script::<Player>().unwrap();
        if player_ref.is_completely_dead(scene) {
            self.sender.as_ref().unwrap().send(Message::EndMatch);
        }
    }

    pub fn update(
        &mut self,
        engine: &mut PluginContext,
        time: GameTime,
        call_button_ui_container: &mut CallButtonUiContainer,
    ) {
        self.time += time.delta;
        let scene = &mut engine.scenes[self.scene];

        self.update_death_zones(scene);
        self.projectiles
            .update(scene, &self.actors, time, self.sender.as_ref().unwrap());
        self.elevators.update(time.delta, scene);
        self.call_buttons
            .update(&self.elevators, call_button_ui_container);
        let mut ctx = UpdateContext {
            time,
            scene,
            items: &self.items,
            doors: &self.doors_container,
            navmesh: self.navmesh,
            elevators: &self.elevators,
            call_buttons: &self.call_buttons,
            sender: self.sender.as_ref().unwrap(),
        };

        self.actors.update(&mut ctx);
        self.trails.update(time.delta, scene);
        self.update_game_ending(scene);
        self.lights.update(scene, time.delta);
        self.triggers
            .update(scene, &self.actors, self.sender.as_ref().unwrap());
        // Make sure to clear unused animation events, because they might be used
        // in next frames which might cause unwanted side effects (like multiple
        // queued attack events can result in huge damage at single frame).
        scene.animations.clear_animation_events();
    }

    fn shoot_ray(
        &mut self,
        engine: &mut PluginContext,
        shooter: Shooter,
        begin: Vector3<f32>,
        end: Vector3<f32>,
        damage: Damage,
        shot_effect: ShotEffect,
    ) {
        let scene = &mut engine.scenes[self.scene];

        // Do immediate intersection test and solve it.
        let (trail_len, hit_point) = if let Some(hit) = ray_hit(
            begin,
            end,
            shooter,
            &self.actors,
            &mut scene.graph,
            Default::default(),
        ) {
            let sender = self.sender.as_ref().unwrap();

            // Just send new messages, instead of doing everything manually here.
            sender.send(Message::CreateEffect {
                kind: if hit.actor.is_some() {
                    EffectKind::BloodSpray
                } else {
                    EffectKind::BulletImpact
                },
                position: hit.position,
                orientation: vector_to_quat(hit.normal),
            });

            sender.send(Message::PlayEnvironmentSound {
                collider: hit.collider,
                feature: hit.feature,
                position: hit.position,
                sound_kind: SoundKind::Impact,
                gain: 1.0,
                rolloff_factor: 1.0,
                radius: 0.5,
            });

            let critical_shot_probability = match shooter {
                Shooter::Weapon(weapon) => {
                    let weapon = weapon_mut(weapon, &mut scene.graph);

                    if hit.actor.is_some() {
                        weapon.set_sight_reaction(SightReaction::HitDetected);
                    }

                    weapon.definition.base_critical_shot_probability
                }
                Shooter::Turret(_) => 0.01,
                _ => 0.0,
            };

            sender.send(Message::DamageActor {
                actor: hit.actor,
                who: hit.who,
                hitbox: hit.hit_box,
                amount: damage
                    .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor))
                    .amount(),
                critical_shot_probability,
            });

            let dir = hit.position - begin;

            let hit_collider_body = scene.graph[hit.collider].parent();
            let parent = if let Some(collider_parent) =
                scene.graph[hit_collider_body].cast_mut::<RigidBody>()
            {
                collider_parent.apply_force_at_point(
                    dir.try_normalize(std::f32::EPSILON)
                        .unwrap_or_default()
                        .scale(30.0),
                    hit.position,
                );
                hit_collider_body
            } else {
                Default::default()
            };

            if hit.actor.is_some() {
                if let Actor::Bot(actor) = self.actors.get_mut(hit.actor) {
                    let body = scene.graph[hit.collider].parent();
                    actor
                        .impact_handler
                        .handle_impact(scene, body, hit.position, dir);
                }
            }

            Decal::new_bullet_hole(
                engine.resource_manager.clone(),
                &mut scene.graph,
                hit.position,
                hit.normal,
                parent,
                if hit.actor.is_some() {
                    Color::opaque(160, 0, 0)
                } else {
                    Color::opaque(20, 20, 20)
                },
            );

            // Add blood splatter on a surface behind an actor that was shot.
            if hit.actor.is_some() && !self.actors.get(hit.actor).is_dead() {
                for intersection in hit.query_buffer.iter() {
                    if matches!(
                        scene.graph[intersection.collider].as_collider().shape(),
                        ColliderShape::Trimesh(_)
                    ) && intersection.position.coords.metric_distance(&hit.position) < 2.0
                    {
                        Decal::add_to_graph(
                            &mut scene.graph,
                            intersection.position.coords,
                            dir,
                            Handle::NONE,
                            Color::opaque(255, 255, 255),
                            Vector3::new(0.45, 0.45, 0.2),
                            engine.resource_manager.request_texture(
                                "data/textures/decals/BloodSplatter_BaseColor.png",
                            ),
                        );

                        break;
                    }
                }
            }

            (dir.norm(), hit.position)
        } else {
            (30.0, end)
        };

        match shot_effect {
            ShotEffect::Smoke => {
                self.trails.add(ShotTrail::new(
                    crate::effects::create(
                        EffectKind::Smoke,
                        &mut scene.graph,
                        engine.resource_manager.clone(),
                        begin,
                        Default::default(),
                    ),
                    5.0,
                ));
            }
            ShotEffect::Beam => {
                let trail_radius = 0.0014;

                let trail = MeshBuilder::new(
                    BaseBuilder::new()
                        .with_cast_shadows(false)
                        .with_local_transform(
                            TransformBuilder::new()
                                .with_local_position(begin)
                                .with_local_scale(Vector3::new(
                                    trail_radius,
                                    trail_radius,
                                    trail_len,
                                ))
                                .with_local_rotation(UnitQuaternion::face_towards(
                                    &(end - begin),
                                    &Vector3::y(),
                                ))
                                .build(),
                        ),
                )
                .with_surfaces(vec![SurfaceBuilder::new(self.beam.clone().unwrap())
                    .with_material(Arc::new(Mutex::new({
                        let mut material = Material::standard();
                        Log::verify(material.set_property(
                            &ImmutableString::new("diffuseColor"),
                            PropertyValue::Color(Color::from_rgba(255, 255, 255, 120)),
                        ));
                        material
                    })))
                    .build()])
                .with_render_path(RenderPath::Forward)
                .build(&mut scene.graph);

                self.trails.add(ShotTrail::new(trail, 0.2));
            }
            ShotEffect::Rail => {
                self.trails.add(ShotTrail::new(
                    crate::effects::create_rail(
                        &mut scene.graph,
                        engine.resource_manager.clone(),
                        begin,
                        hit_point,
                        Color::opaque(255, 0, 0),
                    ),
                    5.0,
                ));
            }
        }
    }

    fn apply_splash_damage(
        &mut self,
        engine: &mut PluginContext,
        amount: f32,
        radius: f32,
        center: Vector3<f32>,
        who: Handle<Actor>,
        critical_shot_probability: f32,
    ) {
        let scene = &mut engine.scenes[self.scene];
        // Just find out actors which must be damaged and re-cast damage message for each.
        for (actor_handle, actor) in self.actors.pair_iter() {
            // TODO: Add occlusion test. This will hit actors through walls.
            let position = actor.position(&scene.graph);
            if position.metric_distance(&center) <= radius {
                self.sender.as_ref().unwrap().send(Message::DamageActor {
                    actor: actor_handle,
                    who,
                    hitbox: None,
                    /// TODO: Maybe collect all hitboxes?
                    amount,
                    critical_shot_probability,
                });
            }
        }
    }

    fn try_open_door(
        &mut self,
        engine: &mut PluginContext,
        door: Handle<Node>,
        actor: Handle<Actor>,
    ) {
        let graph = &mut engine.scenes[self.scene].graph;
        let inventory = self.actors.try_get(actor).map(|a| &a.inventory);
        door_mut(door, graph).try_open(inventory);
    }

    fn call_elevator(&mut self, elevator: Handle<Elevator>, floor: u32) {
        self.elevators[elevator].call_to(floor);
    }

    fn set_call_button_floor(&mut self, call_button: Handle<CallButton>, floor: u32) {
        self.call_buttons[call_button].floor = floor;
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
            &Message::SetCallButtonFloor { call_button, floor } => {
                self.set_call_button_floor(call_button, floor)
            }

            &Message::CallElevator { elevator, floor } => {
                self.call_elevator(elevator, floor);
            }
            &Message::TryOpenDoor { door, actor } => {
                self.try_open_door(engine, door, actor);
            }
            Message::AddBot {
                kind,
                position,
                rotation,
            } => {
                self.add_bot(engine, *kind, *position, *rotation).await;
            }
            &Message::RemoveActor { actor } => self.remove_actor(engine, actor).await,
            &Message::CreateProjectile {
                kind,
                position,
                direction,
                initial_velocity,
                shooter: owner,
            } => {
                self.create_projectile(engine, kind, position, direction, initial_velocity, owner)
                    .await
            }
            &Message::SpawnBot { spawn_point_id } => {
                if let Some(spawn_point) = self.spawn_points.get_mut(spawn_point_id) {
                    spawn_bot(
                        spawn_point,
                        &mut self.actors,
                        engine.resource_manager.clone(),
                        &mut engine.scenes[self.scene],
                        None,
                    )
                    .await;
                }
            }
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
            &Message::DamageActor {
                actor,
                who,
                amount,
                hitbox,
                critical_shot_probability,
            } => {
                self.damage_actor(
                    engine,
                    actor,
                    who,
                    amount,
                    hitbox,
                    critical_shot_probability,
                );
            }
            &Message::CreateEffect {
                kind,
                position,
                orientation,
            } => {
                effects::create(
                    kind,
                    &mut engine.scenes[self.scene].graph,
                    engine.resource_manager.clone(),
                    position,
                    orientation,
                );
            }
            Message::ShootRay {
                shooter: weapon,
                begin,
                end,
                damage,
                shot_effect,
            } => {
                self.shoot_ray(engine, *weapon, *begin, *end, *damage, *shot_effect);
            }
            _ => (),
        }
    }

    pub fn resolve(
        &mut self,
        engine: &mut PluginContext,
        sender: MessageSender,
        display_texture: Texture,
        inventory_texture: Texture,
        item_texture: Texture,
        journal_texture: Texture,
    ) {
        self.set_message_sender(sender);

        self.actors.resolve(
            &mut engine.scenes[self.scene],
            display_texture,
            inventory_texture,
            item_texture,
            journal_texture,
        );

        self.beam = Some(make_beam());
        let scene = &mut engine.scenes[self.scene];
        self.sound_manager.resolve(scene);
        self.projectiles.resolve();
    }

    pub fn set_message_sender(&mut self, sender: MessageSender) {
        self.sender = Some(sender);
    }

    pub fn debug_draw(&self, context: &mut PluginContext) {
        let scene = &mut context.scenes[self.scene];

        let drawing_context = &mut scene.drawing_context;

        drawing_context.clear_lines();

        scene.graph.physics.draw(drawing_context);

        if self.navmesh.is_some() {
            let navmesh = &scene.navmeshes[self.navmesh];

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
                if let Actor::Bot(bot) = actor {
                    bot.debug_draw(drawing_context);
                }
            }
        }

        for death_zone in self.death_zones.iter() {
            drawing_context.draw_aabb(&death_zone.bounds, Color::opaque(0, 0, 200));
        }
    }
}

#[derive(Visit)]
pub struct SpawnPoint {
    position: Vector3<f32>,
    rotation: UnitQuaternion<f32>,
    bot_kind: BotKind,
    spawned: bool,
    with_gun: bool,
}

impl Default for SpawnPoint {
    fn default() -> Self {
        Self {
            position: Default::default(),
            rotation: Default::default(),
            bot_kind: BotKind::Zombie,
            spawned: false,
            with_gun: false,
        }
    }
}