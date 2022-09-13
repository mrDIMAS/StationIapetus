use crate::{
    actor::{Actor, ActorContainer},
    current_level_ref,
    message::Message,
    weapon::definition::ShotEffect,
    weapon::projectile::{Damage, Shooter},
    GameConstructor, MessageSender,
};
use fyrox::{
    core::{
        algebra::{Matrix4, Point3, UnitQuaternion, Vector3},
        arrayvec::ArrayVec,
        color::Color,
        math::{frustum::Frustum, ray::Ray, SmoothAngle, Vector3Ext},
        pool::Handle,
        rand::{seq::SliceRandom, thread_rng},
        visitor::{Visit, VisitResult, Visitor},
    },
    core::{
        inspect::prelude::*,
        reflect::Reflect,
        uuid::{uuid, Uuid},
    },
    impl_component_provider,
    scene::{
        collider::{Collider, ColliderShape, InteractionGroups},
        debug::SceneDrawingContext,
        graph::{map::NodeHandleMap, physics::RayCastOptions},
        light::BaseLight,
        node::{Node, TypeUuidProvider},
        Scene,
    },
    script::{ScriptContext, ScriptTrait},
};
use std::path::PathBuf;
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(
    Copy,
    Clone,
    Hash,
    PartialOrd,
    PartialEq,
    Eq,
    Ord,
    Visit,
    Inspect,
    Reflect,
    AsRefStr,
    EnumString,
    EnumVariantNames,
    Debug,
)]
#[repr(u32)]
pub enum ShootMode {
    /// Turret will shoot from random point every shot.
    Consecutive = 0,
    /// Turret will shoot from each point each shot at once.
    Simultaneously = 1,
}

impl Default for ShootMode {
    fn default() -> Self {
        Self::Consecutive
    }
}

#[derive(
    Copy,
    Clone,
    Hash,
    PartialOrd,
    PartialEq,
    Eq,
    Ord,
    Visit,
    Inspect,
    Reflect,
    AsRefStr,
    EnumString,
    EnumVariantNames,
    Debug,
)]
#[repr(u32)]
pub enum Hostility {
    Player,
    Monsters,
    All,
}

impl Default for Hostility {
    fn default() -> Self {
        Self::Player
    }
}

#[derive(Visit, Reflect, Inspect, Debug, Clone)]
pub struct Turret {
    model: Handle<Node>,
    body: Handle<Node>,
    barrel_stand: Handle<Node>,
    barrels: Vec<Barrel>,
    shoot_mode: ShootMode,
    hostility: Hostility,
    yaw: SmoothAngle,
    pitch: SmoothAngle,
    projector: Handle<Node>,

    #[reflect(hidden)]
    #[inspect(skip)]
    shoot_timer: f32,

    #[reflect(hidden)]
    #[inspect(skip)]
    barrel_index: u32,

    #[reflect(hidden)]
    #[inspect(skip)]
    target_check_timer: f32,

    #[reflect(hidden)]
    #[inspect(skip)]
    #[visit(skip)]
    target: Handle<Actor>,

    #[reflect(hidden)]
    #[inspect(skip)]
    #[visit(skip)]
    frustum: Frustum,
}

impl Default for Turret {
    fn default() -> Self {
        Self {
            body: Default::default(),
            model: Default::default(),
            barrel_stand: Default::default(),
            projector: Default::default(),
            barrels: Default::default(),
            shoot_mode: Default::default(),
            target: Default::default(),
            shoot_timer: Default::default(),
            barrel_index: Default::default(),
            frustum: Default::default(),
            hostility: Default::default(),
            yaw: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 3.0, // rad/s
            },
            pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 3.0, // rad/s
            },
            target_check_timer: 0.0,
        }
    }
}

impl_component_provider!(Turret);

impl TypeUuidProvider for Turret {
    fn type_uuid() -> Uuid {
        uuid!("7a23ce43-500e-4a49-995d-57f44486ed20")
    }
}

impl ScriptTrait for Turret {
    fn on_update(&mut self, context: ScriptContext) {
        let ScriptContext {
            dt,
            plugin,
            handle,
            scene,
            ..
        } = context;

        let level_ref = current_level_ref(plugin).expect("Level must exist!");

        self.update_frustum(scene);

        self.shoot_timer -= dt;
        self.target_check_timer -= dt;

        if self.target_check_timer <= 0.0 {
            self.select_target(scene, &level_ref.actors);
            self.target_check_timer = 0.15;
        }

        if level_ref.actors.contains(self.target) {
            let target_position = level_ref.actors.get(self.target).position(&scene.graph);

            let position = scene.graph[self.model].global_position();

            let d = target_position - position;

            // Aim horizontally.
            let d_model_rel = scene.graph[self.model]
                .global_transform()
                .try_inverse()
                .unwrap_or_default()
                .transform_vector(&d);
            self.yaw.set_target(d_model_rel.x.atan2(d_model_rel.z));

            // Aim vertically.
            if let Some(d_body_rel) = scene.graph[self.body]
                .global_transform()
                .try_inverse()
                .unwrap_or_default()
                .transform_vector(&d)
                .try_normalize(f32::EPSILON)
            {
                self.pitch.set_target(d_body_rel.dot(&Vector3::y()).acos());
            }

            if self.shoot_timer <= 0.0 {
                self.shoot_timer = 0.1;

                match self.shoot_mode {
                    ShootMode::Consecutive => {
                        if let Some(barrel) = self.barrels.get_mut(self.barrel_index as usize) {
                            barrel.shoot(
                                handle,
                                scene,
                                target_position,
                                level_ref.sender.as_ref().unwrap(),
                            );
                            self.barrel_index += 1;
                            if self.barrel_index >= self.barrels.len() as u32 {
                                self.barrel_index = 0;
                            }
                        }
                    }
                    ShootMode::Simultaneously => {
                        for barrel in self.barrels.iter_mut() {
                            barrel.shoot(
                                handle,
                                scene,
                                target_position,
                                level_ref.sender.as_ref().unwrap(),
                            );
                        }
                    }
                }
            }

            for barrel in self.barrels.iter_mut() {
                barrel.update(scene);
            }

            if self.projector.is_some() {
                scene.graph[self.projector]
                    .query_component_mut::<BaseLight>()
                    .unwrap()
                    .set_color(Color::opaque(255, 0, 0));
            }
        } else {
            self.pitch.set_target(90.0f32.to_radians());
            self.yaw
                .set_target(self.yaw.angle() + 50.0f32.to_radians() * dt);

            if self.projector.is_some() {
                scene.graph[self.projector]
                    .query_component_mut::<BaseLight>()
                    .unwrap()
                    .set_color(Color::opaque(255, 127, 40));
            }
        }

        self.pitch.update(dt);
        self.yaw.update(dt);

        scene.graph[self.body]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(
                &Vector3::y_axis(),
                90.0f32.to_radians() + self.yaw.angle(),
            ));
        scene.graph[self.barrel_stand]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(
                &Vector3::z_axis(),
                self.pitch.angle() - std::f32::consts::FRAC_PI_2,
            ));
    }

    fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping
            .map(&mut self.barrel_stand)
            .map(&mut self.body)
            .map(&mut self.model)
            .map(&mut self.projector);

        for barrel in self.barrels.iter_mut() {
            barrel.remap_handles(old_new_mapping);
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }

    fn plugin_uuid(&self) -> Uuid {
        GameConstructor::type_uuid()
    }
}

#[derive(Default, Visit, Reflect, Inspect, Clone, Debug)]
pub struct Barrel {
    handle: Handle<Node>,
    shoot_point: Handle<Node>,

    #[reflect(hidden)]
    #[inspect(skip)]
    initial_position: Vector3<f32>,

    #[reflect(hidden)]
    #[inspect(skip)]
    offset: Vector3<f32>,
}

impl Barrel {
    fn shoot(
        &mut self,
        owner_handle: Handle<Node>,
        scene: &Scene,
        target_position: Vector3<f32>,
        sender: &MessageSender,
    ) {
        self.offset = Vector3::new(-20.0, 0.0, 0.0);

        let shoot_point = &scene.graph[self.shoot_point];

        sender.send(Message::ShootRay {
            shooter: Shooter::Turret(owner_handle),
            begin: shoot_point.global_position(),
            end: target_position,
            damage: Damage::Point(10.0),
            shot_effect: ShotEffect::Smoke,
        });

        let sounds = [
            "data/sounds/turret_shot_1.ogg",
            "data/sounds/turret_shot_2.ogg",
            "data/sounds/turret_shot_3.ogg",
        ];

        sender.send(Message::PlaySound {
            path: PathBuf::from(sounds.choose(&mut thread_rng()).unwrap()),
            position: shoot_point.global_position(),
            gain: 1.0,
            rolloff_factor: 1.0,
            radius: 3.0,
        });
    }

    fn update(&mut self, scene: &mut Scene) {
        self.offset.follow(&Vector3::default(), 0.4);

        scene.graph[self.handle]
            .local_transform_mut()
            .set_position(self.initial_position + self.offset);
    }

    fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping
            .map(&mut self.handle)
            .map(&mut self.shoot_point);
    }
}

impl Turret {
    pub fn debug_draw(&self, context: &mut SceneDrawingContext) {
        context.draw_frustum(&self.frustum, Color::from_rgba(0, 200, 0, 255));
    }

    fn update_frustum(&mut self, scene: &Scene) {
        let barrel_stand = &scene.graph[self.barrel_stand];
        let up = barrel_stand.up_vector();
        let look_at = barrel_stand.global_position() - barrel_stand.side_vector();
        let view_matrix = Matrix4::look_at_rh(
            &Point3::from(barrel_stand.global_position()),
            &Point3::from(look_at),
            &up,
        );
        let projection_matrix =
            Matrix4::new_perspective(16.0 / 9.0, 90.0f32.to_radians(), 0.1, 5.0);
        self.frustum = Frustum::from(projection_matrix * view_matrix).unwrap();
    }

    fn select_target(&mut self, scene: &Scene, actors: &ActorContainer) {
        let self_position = scene.graph[self.model].global_position();

        let self_collider = scene.graph[scene.graph[self.model].parent()]
            .children()
            .iter()
            .filter(|h| scene.graph[**h].is_collider())
            .next()
            .cloned();

        if !actors.contains(self.target) || !actors.get(self.target).is_dead() {
            let mut closest = Handle::NONE;
            let mut closest_distance = f32::MAX;
            'target_loop: for (handle, actor) in actors.pair_iter() {
                if actor.is_dead() {
                    continue 'target_loop;
                }

                let is_player = matches!(actor, Actor::Player(_));
                if self.hostility == Hostility::Player && !is_player
                    || self.hostility == Hostility::Monsters && is_player
                {
                    continue;
                }

                let mut query_buffer = ArrayVec::<_, 128>::new();

                let actor_position = actor.position(&scene.graph);

                if !self.frustum.is_contains_point(actor_position) {
                    continue 'target_loop;
                }

                let ray = Ray::from_two_points(actor_position, self_position);
                scene.graph.physics.cast_ray(
                    RayCastOptions {
                        ray_origin: Point3::from(ray.origin),
                        ray_direction: ray.dir,
                        groups: InteractionGroups::default(),
                        max_len: ray.dir.norm(),
                        sort_results: true,
                    },
                    &mut query_buffer,
                );

                'hit_loop: for hit in query_buffer.iter() {
                    if let Some(self_collider) = self_collider {
                        if self_collider == hit.collider {
                            continue 'hit_loop;
                        }
                    }

                    if let Some(collider) = &scene.graph[hit.collider].cast::<Collider>() {
                        if !matches!(collider.shape(), ColliderShape::Capsule(_)) {
                            self.target = Default::default();
                            // Target is behind something.
                            continue 'target_loop;
                        }
                    }
                }

                let distance = actor_position.metric_distance(&self_position);
                if distance < closest_distance {
                    closest_distance = distance;
                    closest = handle;
                }
            }
            self.target = closest;
        } else if actors.get(self.target).is_dead() {
            self.target = Default::default();
        }
    }
}
