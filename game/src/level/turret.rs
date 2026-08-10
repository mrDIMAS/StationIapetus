use crate::bot::Bot;
use crate::{sound::SoundManager, weapon::projectile::Projectile, Game, Player};
use fyrox::{
    core::{
        algebra::{Matrix4, Point3, Vector3},
        color::Color,
        math::{frustum::Frustum, SmoothAngle, Vector3Ext},
        pool::Handle,
        rand::{seq::SliceRandom, thread_rng},
        reflect::prelude::*,
        type_traits::{ComponentProvider, TypeUuidProvider},
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::{Visit, VisitResult, Visitor},
    },
    graph::SceneGraph,
    plugin::error::GameResult,
    resource::model::ModelResource,
    scene::{collider::Collider, debug::SceneDrawingContext, light::BaseLight, node::Node, Scene},
    script::{ScriptContext, ScriptTrait},
};
use strum_macros::{AsRefStr, EnumString, VariantNames};

#[derive(
    Copy,
    Clone,
    Hash,
    PartialOrd,
    PartialEq,
    Eq,
    Ord,
    Visit,
    Reflect,
    AsRefStr,
    EnumString,
    VariantNames,
    Default,
    Debug,
    TypeUuidProvider,
)]
#[repr(u32)]
#[type_uuid(id = "0deea5b2-dad8-418f-be2d-899c4851c76b")]
pub enum ShootMode {
    /// Turret will shoot from random point every shot.
    #[default]
    Consecutive = 0,
    /// Turret will shoot from each point each shot at once.
    Simultaneously = 1,
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
    Reflect,
    AsRefStr,
    EnumString,
    VariantNames,
    Debug,
    Default,
    TypeUuidProvider,
)]
#[repr(u32)]
#[type_uuid(id = "bb2b6799-128a-489f-9d72-e82cc706b228")]
pub enum Hostility {
    #[default]
    Player,
    Monsters,
    All,
}

#[derive(Visit, PartialEq, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "7a23ce43-500e-4a49-995d-57f44486ed20")]
#[visit(optional)]
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
    collider: InheritableVariable<Handle<Collider>>,
    shoot_interval: f32,

    #[reflect(hidden)]
    shoot_timer: f32,

    #[reflect(hidden)]
    barrel_index: u32,

    #[reflect(hidden)]
    target_check_timer: f32,

    #[reflect(hidden)]
    #[visit(skip)]
    target: Handle<Node>,

    #[reflect(hidden)]
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
            collider: Default::default(),
            shoot_interval: 0.2,
        }
    }
}

impl ScriptTrait for Turret {
    fn on_update(&mut self, ctx: &mut ScriptContext) -> GameResult {
        let level_ref = ctx
            .plugins
            .get::<Game>()
            .level
            .as_ref()
            .expect("Level must exist!");

        self.update_frustum(ctx.scene);

        self.shoot_timer -= ctx.dt;
        self.target_check_timer -= ctx.dt;

        if self.target_check_timer <= 0.0 {
            self.select_target(ctx.scene, &level_ref.actors)?;
            self.target_check_timer = 0.15;
        }

        // Ambil target_position dari Player atau Bot (karena keduanya Deref ke Character)
        let target_position = if let Ok(player) = ctx
            .scene
            .graph
            .try_get_script_component_of::<Player>(self.target)
        {
            Some(player.most_vulnerable_point(&ctx.scene.graph))
        } else if let Ok(bot) = ctx
            .scene
            .graph
            .try_get_script_component_of::<Bot>(self.target)
        {
            Some(bot.most_vulnerable_point(&ctx.scene.graph))
        } else {
            None
        };

        if let Some(target_position) = target_position {
            let position = ctx.scene.graph.try_get(self.model)?.global_position();

            let d = target_position - position;

            // Aim horizontally.
            let d_model_rel = ctx
                .scene
                .graph
                .try_get(self.model)?
                .global_transform()
                .try_inverse()
                .unwrap_or_default()
                .transform_vector(&d);
            self.yaw.set_target(d_model_rel.x.atan2(d_model_rel.z));

            // Aim vertically.
            if let Some(d_body_rel) = ctx
                .scene
                .graph
                .try_get(self.body)?
                .global_transform()
                .try_inverse()
                .unwrap_or_default()
                .transform_vector(&d)
                .try_normalize(f32::EPSILON)
            {
                self.pitch.set_target(d_body_rel.dot(&Vector3::y()).acos());
            }

            if self.shoot_timer <= 0.0 {
                self.shoot_timer = self.shoot_interval;

                match self.shoot_mode {
                    ShootMode::Consecutive => {
                        if let Some(barrel) = self.barrels.get_mut(self.barrel_index as usize) {
                            barrel.shoot(
                                ctx.handle,
                                ctx.scene,
                                target_position,
                                &level_ref.sound_manager,
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
                                ctx.handle,
                                ctx.scene,
                                target_position,
                                &level_ref.sound_manager,
                            );
                        }
                    }
                }
            }

            for barrel in self.barrels.iter_mut() {
                barrel.update(ctx.scene);
            }
        } else {
            self.pitch.set_target(90.0f32.to_radians());
            self.yaw
                .set_target(self.yaw.angle() + 50.0f32.to_radians() * ctx.dt);
        }
        ctx.scene
            .graph
            .try_get_mut_of_type::<BaseLight>(self.projector)?
            .set_color(if self.target.is_some() {
                Color::opaque(255, 0, 0)
            } else {
                Color::opaque(255, 127, 40)
            });

        self.pitch.update(ctx.dt);
        self.yaw.update(ctx.dt);

        ctx.scene
            .graph
            .try_get_mut(self.body)?
            .set_rotation_y(90.0f32.to_radians() + self.yaw.angle());
        ctx.scene
            .graph
            .try_get_mut(self.barrel_stand)?
            .set_rotation_z(self.pitch.angle() - std::f32::consts::FRAC_PI_2);
        Ok(())
    }
}

#[derive(Default, Visit, Reflect, Clone, PartialEq, Debug, TypeUuidProvider)]
#[visit(optional)]
#[type_uuid(id = "d32845ee-62f3-4073-8675-623aa2ab0644")]
pub struct Barrel {
    handle: Handle<Node>,
    shoot_point: Handle<Node>,
    projectile: Option<ModelResource>,
    #[reflect(hidden)]
    initial_position: Vector3<f32>,
    #[reflect(hidden)]
    offset: Vector3<f32>,
}

impl Barrel {
    fn shoot(
        &mut self,
        owner_handle: Handle<Node>,
        scene: &mut Scene,
        target_position: Vector3<f32>,
        sound_manager: &SoundManager,
    ) {
        self.offset = Vector3::new(-20.0, 0.0, 0.0);

        let shot_position = scene.graph[self.shoot_point].global_position();

        if let Some(projectile) = self.projectile.as_ref() {
            Projectile::spawn(
                projectile,
                scene,
                target_position - shot_position,
                shot_position,
                owner_handle,
                Default::default(),
            );
        }

        let sounds = [
            "data/sounds/turret_shot_1.ogg",
            "data/sounds/turret_shot_2.ogg",
            "data/sounds/turret_shot_3.ogg",
        ];

        sound_manager.play_sound(
            &mut scene.graph,
            sounds.choose(&mut thread_rng()).unwrap(),
            shot_position,
            1.0,
            1.0,
            3.0,
        );
    }

    fn update(&mut self, scene: &mut Scene) {
        self.offset.follow(&Vector3::default(), 0.4);

        scene.graph[self.handle]
            .local_transform_mut()
            .set_position(self.initial_position + self.offset);
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
        self.frustum =
            Frustum::from_view_projection_matrix(projection_matrix * view_matrix).unwrap();
    }

    fn select_target(&mut self, scene: &Scene, actors: &[Handle<Node>]) -> GameResult {
        let mut closest_distance = f32::MAX;
        let mut closest_target = Handle::NONE;

        let position = scene.graph.try_get(self.model)?.global_position();

        'target_loop: for &handle in actors {
            // Cek apakah actor adalah Player atau Bot
            let is_player = scene
                .graph
                .try_get_script_component_of::<Player>(handle)
                .is_ok();

            let is_bot = scene
                .graph
                .try_get_script_component_of::<Bot>(handle)
                .is_ok();

            if !is_player && !is_bot {
                continue 'target_loop;
            }

            // Filter hostility
            if (self.hostility == Hostility::Player && !is_player)
                || (self.hostility == Hostility::Monsters && !is_bot)
            {
                continue 'target_loop;
            }

            // Ambil target_position dan status is_dead dari Player atau Bot
            let (target_position, is_dead) =
                if let Ok(player) = scene.graph.try_get_script_component_of::<Player>(handle) {
                    (
                        player.most_vulnerable_point(&scene.graph),
                        player.is_dead(&scene.graph),
                    )
                } else if let Ok(bot) = scene.graph.try_get_script_component_of::<Bot>(handle) {
                    (
                        bot.most_vulnerable_point(&scene.graph),
                        bot.is_dead(&scene.graph),
                    )
                } else {
                    continue 'target_loop;
                };

            if is_dead {
                continue 'target_loop;
            }

            if !self.frustum.is_contains_point(target_position) {
                continue 'target_loop;
            }

            let distance = position.metric_distance(&target_position);
            if distance < closest_distance {
                closest_distance = distance;
                closest_target = handle;
            }
        }

        self.target = closest_target;

        Ok(())
    }
}
