use crate::actor::ActorContainer;
use crate::weapon::projectile::Damage;
use crate::{actor::Actor, message::Message, weapon::projectile::Shooter};
use rg3d::core::algebra::{Matrix4, Point3, UnitQuaternion, Vector3};
use rg3d::core::math::frustum::Frustum;
use rg3d::core::math::Vector3Ext;
use rg3d::core::rand::seq::SliceRandom;
use rg3d::core::rand::thread_rng;
use rg3d::{
    core::{
        pool::{Handle, Pool},
        visitor::{Visit, VisitResult, Visitor},
    },
    scene::{node::Node, Scene},
};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

#[derive(Copy, Clone, Hash, PartialOrd, PartialEq, Eq, Ord)]
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

impl ShootMode {
    fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::Consecutive),
            1 => Ok(Self::Simultaneously),
            _ => Err(format!("Invalid shoot mode id {}!", id)),
        }
    }
}

impl Visit for ShootMode {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        let mut id = *self as u32;
        id.visit(name, visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(id)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Turret {
    model: Handle<Node>,
    body: Handle<Node>,
    barrel_stand: Handle<Node>,
    barrels: Vec<Barrel>,
    shoot_mode: ShootMode,
    target: Handle<Actor>,
    shoot_timer: f32,
    barrel_index: u32,
    frustum: Frustum,
}

impl Visit for Turret {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.model.visit("Model", visitor)?;
        self.body.visit("Body", visitor)?;
        self.barrels.visit("Barrels", visitor)?;
        self.barrel_stand.visit("BarrelStand", visitor)?;
        self.shoot_mode.visit("ShootMode", visitor)?;
        self.target.visit("Target", visitor)?;
        self.barrel_index.visit("BarrelIndex", visitor)?;
        self.shoot_timer.visit("ShootTimer", visitor)?;

        visitor.leave_region()
    }
}

#[derive(Default)]
pub struct Barrel {
    handle: Handle<Node>,
    shoot_point: Handle<Node>,
    initial_position: Vector3<f32>,
    offset: Vector3<f32>,
}

impl Visit for Barrel {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.handle.visit("Handle", visitor)?;
        self.shoot_point.visit("ShootPoint", visitor)?;
        self.offset.visit("Offset", visitor)?;
        self.initial_position.visit("InitialPosition", visitor)?;

        visitor.leave_region()
    }
}

impl Barrel {
    fn shoot(
        &mut self,
        owner_handle: Handle<Turret>,
        scene: &Scene,
        target_position: Vector3<f32>,
        sender: &Sender<Message>,
    ) {
        self.offset = Vector3::new(-20.0, 0.0, 0.0);

        let shoot_point = &scene.graph[self.shoot_point];

        sender
            .send(Message::ShootRay {
                shooter: Shooter::Turret(owner_handle),
                begin: shoot_point.global_position(),
                end: target_position,
                damage: Damage::Point(0.0),
            })
            .unwrap();

        let sounds = [
            "data/sounds/turret_shot_1.ogg",
            "data/sounds/turret_shot_2.ogg",
            "data/sounds/turret_shot_3.ogg",
        ];

        sender
            .send(Message::PlaySound {
                path: PathBuf::from(sounds.choose(&mut thread_rng()).unwrap()),
                position: shoot_point.global_position(),
                gain: 1.0,
                rolloff_factor: 1.0,
                radius: 1.0,
            })
            .unwrap();
    }

    fn update(&mut self, scene: &mut Scene) {
        self.offset.follow(&Vector3::default(), 0.4);

        scene.graph[self.handle]
            .local_transform_mut()
            .set_position(self.initial_position + self.offset);
    }
}

impl Turret {
    pub async fn new(model: Handle<Node>, scene: &Scene, shoot_mode: ShootMode) -> Self {
        let stand = scene.graph.find_by_name(model, "Body");
        let barrel_stand = scene.graph.find_by_name(model, "BarrelStand");

        Self {
            body: stand,
            model,
            barrel_stand,
            barrels: scene
                .graph
                .traverse_handle_iter(barrel_stand)
                .filter_map(|h| {
                    if h != barrel_stand && scene.graph[h].name().starts_with("Barrel") {
                        Some(Barrel {
                            handle: h,
                            shoot_point: scene
                                .graph
                                .find(h, &mut |n| n.name().starts_with("ShootPoint")),
                            offset: Default::default(),
                            initial_position: **scene.graph[h].local_transform().position(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            shoot_mode,
            target: Default::default(),
            shoot_timer: 0.0,
            barrel_index: 0,
            frustum: Default::default(),
        }
    }

    fn update_frustum(&mut self, scene: &Scene) {
        let barrel_stand = &scene.graph[self.barrel_stand];
        let up = barrel_stand.up_vector();
        let look_at = barrel_stand.global_position()
            + UnitQuaternion::from_axis_angle(&Vector3::y_axis(), -90.0f32.to_radians())
                .transform_vector(&barrel_stand.look_vector());
        let view_matrix = Matrix4::look_at_rh(
            &Point3::from(barrel_stand.global_position()),
            &Point3::from(look_at),
            &up,
        );
        let projection_matrix =
            Matrix4::new_perspective(16.0 / 9.0, 90.0f32.to_radians(), 0.1, 5.0);
        let view_projection_matrix = projection_matrix * view_matrix;
        self.frustum = Frustum::from(view_projection_matrix).unwrap();
    }

    fn select_target(&mut self, scene: &Scene, actors: &ActorContainer) {
        let position = scene.graph[self.model].global_position();

        if !actors.contains(self.target) || !actors.get(self.target).is_dead() {
            let mut closest = Handle::NONE;
            let mut closest_distance = f32::MAX;
            for (handle, actor) in actors.pair_iter() {
                let distance = actor.position(&scene.graph).metric_distance(&position);
                if distance < closest_distance {
                    closest_distance = distance;
                    closest = handle;
                }
            }
            if closest.is_some() {
                self.target = closest;
            }
        }
    }

    fn update(
        &mut self,
        self_handle: Handle<Self>,
        scene: &mut Scene,
        actors: &ActorContainer,
        sender: &Sender<Message>,
        dt: f32,
    ) {
        self.select_target(scene, actors);
        self.update_frustum(scene);

        self.shoot_timer -= dt;

        if actors.contains(self.target) {
            let target_position = actors.get(self.target).position(&scene.graph);

            if self.frustum.is_contains_point(target_position) {
                let position = scene.graph[self.model].global_position();

                let d = target_position - position;

                // Aim horizontally.
                let d_model_rel = scene.graph[self.model]
                    .global_transform()
                    .try_inverse()
                    .unwrap_or_default()
                    .transform_vector(&d);

                let yaw = d_model_rel.x.atan2(d_model_rel.z);

                scene.graph[self.body].local_transform_mut().set_rotation(
                    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 90.0f32.to_radians() + yaw),
                );

                // Aim vertically.
                if let Some(d_body_rel) = scene.graph[self.body]
                    .global_transform()
                    .try_inverse()
                    .unwrap_or_default()
                    .transform_vector(&d)
                    .try_normalize(f32::EPSILON)
                {
                    let pitch = d_body_rel.dot(&Vector3::y()).acos();

                    scene.graph[self.barrel_stand]
                        .local_transform_mut()
                        .set_rotation(UnitQuaternion::from_axis_angle(
                            &Vector3::z_axis(),
                            pitch - std::f32::consts::FRAC_PI_2,
                        ));
                }

                if self.shoot_timer <= 0.0 {
                    self.shoot_timer = 0.1;

                    match self.shoot_mode {
                        ShootMode::Consecutive => {
                            if let Some(barrel) = self.barrels.get_mut(self.barrel_index as usize) {
                                barrel.shoot(self_handle, scene, target_position, sender);
                                self.barrel_index += 1;
                                if self.barrel_index >= self.barrels.len() as u32 {
                                    self.barrel_index = 0;
                                }
                            }
                        }
                        ShootMode::Simultaneously => {
                            for barrel in self.barrels.iter_mut() {
                                barrel.shoot(self_handle, scene, target_position, sender);
                            }
                        }
                    }
                }

                for barrel in self.barrels.iter_mut() {
                    barrel.update(scene);
                }
            }
        }
    }
}

#[derive(Default)]
pub struct TurretContainer {
    pool: Pool<Turret>,
}

impl TurretContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, turret: Turret) -> Handle<Turret> {
        self.pool.spawn(turret)
    }

    pub fn update(
        &mut self,
        scene: &mut Scene,
        actors: &ActorContainer,
        sender: &Sender<Message>,
        dt: f32,
    ) {
        for (self_handle, turret) in self.pool.pair_iter_mut() {
            turret.update(self_handle, scene, actors, sender, dt);
        }
    }
}

impl Visit for TurretContainer {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.pool.visit("Pool", visitor)?;

        visitor.leave_region()
    }
}
