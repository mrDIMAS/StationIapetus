use crate::{
    bot::{try_get_bot_mut, Bot, BotCommand},
    character::{character_ref, try_get_character_mut, try_get_character_ref, CharacterCommand},
    config::SoundConfig,
    door::DoorContainer,
    effects::{self, EffectKind},
    elevator::{
        call_button::{CallButton, CallButtonContainer, CallButtonKind},
        Elevator, ElevatorContainer,
    },
    item::ItemContainer,
    level::{
        decal::Decal,
        trail::ShotTrail,
        trigger::{Trigger, TriggerContainer, TriggerKind},
    },
    message::Message,
    player::Player,
    sound::{SoundKind, SoundManager},
    utils::use_hrtf,
    weapon::{
        definition::ShotEffect,
        projectile::{Damage, Projectile, ProjectileContainer, ProjectileKind, Shooter},
        ray_hit,
        sight::SightReaction,
        weapon_mut,
    },
    CallButtonUiContainer, MessageSender,
};
use fyrox::{
    core::{
        algebra::{Point3, UnitQuaternion, Vector3},
        color::Color,
        futures::executor::block_on,
        math::{ray::Ray, vector_to_quat, PositionProvider},
        parking_lot::Mutex,
        pool::Handle,
        sstorage::ImmutableString,
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    material::{Material, PropertyValue},
    plugin::PluginContext,
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
    script::Script,
    utils::log::Log,
};
use std::{path::Path, sync::Arc};

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
    pub projectiles: ProjectileContainer,
    pub actors: Vec<Handle<Node>>,
    pub items: ItemContainer,
    time: f32,
    sound_manager: SoundManager,
    pub doors_container: DoorContainer,
    triggers: TriggerContainer,
    pub elevators: ElevatorContainer,
    pub call_buttons: CallButtonContainer,

    #[visit(skip)]
    sender: Option<MessageSender>,
    #[visit(skip)]
    beam: Option<Arc<Mutex<SurfaceData>>>,
}

#[derive(Default)]
pub struct AnalysisResult {
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

    let mut triggers = TriggerContainer::default();
    let mut elevators = ElevatorContainer::new();
    let mut call_buttons = CallButtonContainer::new();

    for (handle, node) in scene.graph.pair_iter() {
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
            "NextLevelTrigger" => triggers.add(Trigger::new(handle, TriggerKind::NextLevel)),
            "EndGameTrigger" => triggers.add(Trigger::new(handle, TriggerKind::EndGame)),
            _ => (),
        }
    }

    result.triggers = triggers;
    result.elevators = elevators;
    result.call_buttons = call_buttons;

    result
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

        let AnalysisResult {
            triggers,
            elevators,
            call_buttons,
        } = block_on(analyze(scene));

        Self {
            player: Default::default(),
            actors: Default::default(),
            items: Default::default(),
            triggers,
            scene: scene_handle,
            sender: Some(sender),
            time: 0.0,
            projectiles: ProjectileContainer::new(),
            sound_manager: SoundManager::new(scene),
            beam: Some(make_beam()),
            doors_container: Default::default(),
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
            triggers,
            elevators,
            call_buttons,
        } = analyze(&mut scene).await;

        let level = Self {
            player: Default::default(),
            actors: Default::default(),
            items: Default::default(),

            triggers,
            scene: Handle::NONE, // Filled when scene will be moved to engine.
            sender: Some(sender),
            time: 0.0,
            projectiles: ProjectileContainer::new(),
            sound_manager: SoundManager::new(&mut scene),
            beam: Some(make_beam()),
            doors_container: Default::default(),
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

    fn update_game_ending(&self, scene: &Scene) {
        if let Some(player_ref) = scene
            .graph
            .try_get(self.player)
            .and_then(|p| p.try_get_script::<Player>())
        {
            if player_ref.is_completely_dead(scene) {
                self.sender.as_ref().unwrap().send(Message::EndMatch);
            }
        }
    }

    pub fn update(
        &mut self,
        ctx: &mut PluginContext,
        call_button_ui_container: &mut CallButtonUiContainer,
    ) {
        self.time += ctx.dt;
        let scene = &mut ctx.scenes[self.scene];

        self.projectiles
            .update(scene, ctx.dt, &self.actors, self.sender.as_ref().unwrap());
        self.elevators.update(ctx.dt, scene);
        self.call_buttons
            .update(&self.elevators, call_button_ui_container);

        self.update_game_ending(scene);
        self.triggers
            .update(scene, &self.actors, self.sender.as_ref().unwrap());
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

            if let Some(character) = try_get_character_mut(hit.actor, &mut scene.graph) {
                character.push_command(CharacterCommand::Damage {
                    who: hit.who,
                    hitbox: hit.hit_box,
                    amount: damage
                        .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor))
                        .amount(),
                    critical_shot_probability,
                });
            }

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

            if try_get_bot_mut(hit.actor, &mut scene.graph).is_some() {
                let body = scene.graph[hit.collider].parent();
                try_get_bot_mut(hit.actor, &mut scene.graph)
                    .unwrap()
                    .commands_queue
                    .push_back(BotCommand::HandleImpact {
                        handle: body,
                        impact_point: hit.position,
                        direction: dir,
                    });
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
            if !try_get_character_ref(hit.actor, &scene.graph).map_or(true, |a| a.is_dead()) {
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
                let effect = effects::create(
                    EffectKind::Smoke,
                    &mut scene.graph,
                    engine.resource_manager.clone(),
                    begin,
                    Default::default(),
                );
                scene.graph[effect].set_script(Some(Script::new(ShotTrail::new(5.0))));
            }
            ShotEffect::Beam => {
                let trail_radius = 0.0014;
                MeshBuilder::new(
                    BaseBuilder::new()
                        .with_script(Script::new(ShotTrail::new(0.2)))
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
            }
            ShotEffect::Rail => {
                let effect = effects::create_rail(
                    &mut scene.graph,
                    engine.resource_manager.clone(),
                    begin,
                    hit_point,
                    Color::opaque(255, 0, 0),
                );
                scene.graph[effect].set_script(Some(Script::new(ShotTrail::new(5.0))));
            }
        }
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

    pub fn resolve(&mut self, engine: &mut PluginContext, sender: MessageSender) {
        self.set_message_sender(sender);
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
