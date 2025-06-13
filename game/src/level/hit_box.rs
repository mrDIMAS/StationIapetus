use crate::{
    character::{DamageDealer, DamagePosition},
    Game,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        math::vector_to_quat,
        pool::Handle,
        reflect::prelude::*,
        type_traits::prelude::*,
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    graph::SceneGraph,
    resource::model::{ModelResource, ModelResourceExtension},
    scene::{
        collider::{Collider, ColliderShape},
        graph::physics::RayCastOptions,
        node::Node,
        rigidbody::RigidBody,
    },
    script::{
        RoutingStrategy, ScriptContext, ScriptDeinitContext, ScriptMessageContext,
        ScriptMessagePayload, ScriptTrait,
    },
};
use strum_macros::{AsRefStr, EnumString, VariantNames};

#[derive(Debug, Clone)]
pub struct HitBoxDamage {
    pub hit_box: Handle<Node>,
    pub damage: f32,
    pub dealer: DamageDealer,
    pub position: Option<DamagePosition>,
    pub is_melee: bool,
}

#[derive(Debug, Clone)]
pub struct HitBoxHeal {
    pub hit_box: Handle<Node>,
    pub amount: f32,
}

#[derive(Debug, ScriptMessagePayload)]
pub enum HitBoxMessage {
    Damage(HitBoxDamage),
    Heal(HitBoxHeal),
}

#[derive(
    Default,
    Copy,
    Clone,
    PartialOrd,
    PartialEq,
    Eq,
    Ord,
    Hash,
    Debug,
    Visit,
    Reflect,
    AsRefStr,
    EnumString,
    VariantNames,
    TypeUuidProvider,
)]
#[type_uuid(id = "009bccb6-42e4-4dc6-bb26-6a8a70b3fab9")]
pub enum LimbType {
    Leg,
    Arm,
    Head,
    #[default]
    Body,
}

impl LimbType {
    pub fn can_be_sliced_off(&self) -> bool {
        !matches!(self, Self::Body)
    }
}

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "28a107ad-ee88-4a0f-8b32-be348e779115")]
#[visit(optional)]
pub struct HitBox {
    pub bone: InheritableVariable<Handle<Node>>,
    pub damage_factor: InheritableVariable<f32>,
    pub movement_speed_factor: InheritableVariable<f32>,
    #[reflect(description = "An effect prefab that will be spawned by a non-melee hit.")]
    pub hit_prefab: InheritableVariable<Option<ModelResource>>,
    #[reflect(description = "An effect prefab that will be spawned by a melee hit.")]
    pub melee_hit_prefab: InheritableVariable<Option<ModelResource>>,
    #[reflect(
        description = "A prefab that will be spawned behind the hit box at certain distance \
        on hit (melee or not). Could be used for blood splatters."
    )]
    pub pierce_prefab: InheritableVariable<Option<ModelResource>>,
    #[reflect(
        description = "A prefab that will be spawned at the point of impact. Could be used for \
        bullet holes or to add damage decals. It will also be attached to the hit box."
    )]
    pub damage_prefab: InheritableVariable<Option<ModelResource>>,
    #[reflect(
        description = "A prefab that will be spawned at the point of impact if the hit box is about \
        to be destroyed. Could be used to spawn various visual effects."
    )]
    pub destruction_prefab: InheritableVariable<Option<ModelResource>>,
    pub health: InheritableVariable<f32>,
    pub limb_type: InheritableVariable<LimbType>,
    pub environment_damage_timeout: f32,
    pub children_hit_boxes: InheritableVariable<Vec<Handle<Node>>>,
    pub critical_for_survival: InheritableVariable<bool>,
}

impl Default for HitBox {
    fn default() -> Self {
        Self {
            bone: Default::default(),
            damage_factor: 1.0.into(),
            movement_speed_factor: 1.0.into(),
            hit_prefab: Default::default(),
            melee_hit_prefab: Default::default(),
            pierce_prefab: Default::default(),
            damage_prefab: Default::default(),
            destruction_prefab: Default::default(),
            health: 100.0.into(),
            limb_type: Default::default(),
            environment_damage_timeout: 0.0,
            children_hit_boxes: Default::default(),
            critical_for_survival: Default::default(),
        }
    }
}

impl HitBox {
    pub fn is_sliced_off(&self) -> bool {
        self.limb_type.can_be_sliced_off() && *self.health <= 0.0
    }

    fn handle_environment_interaction(&mut self, ctx: &mut ScriptContext) {
        if self.environment_damage_timeout > 0.0 {
            self.environment_damage_timeout -= ctx.dt;
            return;
        }

        let graph = &ctx.scene.graph;

        let Some(collider) = graph.try_get_of_type::<Collider>(ctx.handle) else {
            return;
        };

        'contact_loop: for contact in collider.contacts(&graph.physics) {
            if !contact.has_any_active_contact {
                continue;
            }

            for manifold in contact.manifolds.iter() {
                let Some(rb1) = graph.try_get_of_type::<RigidBody>(manifold.rigid_body1) else {
                    continue;
                };
                let Some(rb2) = graph.try_get_of_type::<RigidBody>(manifold.rigid_body2) else {
                    continue;
                };

                for point in manifold.points.iter() {
                    let hit_strength = (rb1.lin_vel() - rb2.lin_vel()).norm();

                    if hit_strength > 5.0 {
                        ctx.message_sender.send_hierarchical(
                            ctx.handle,
                            RoutingStrategy::Up,
                            HitBoxMessage::Damage(HitBoxDamage {
                                hit_box: ctx.handle,
                                damage: hit_strength,
                                dealer: DamageDealer::default(),
                                position: Some(DamagePosition {
                                    point: graph[contact.collider1]
                                        .global_transform()
                                        .transform_point(&Point3::from(point.local_p1))
                                        .coords,
                                    direction: manifold.normal,
                                }),
                                is_melee: true,
                            }),
                        );

                        self.environment_damage_timeout = 0.25;

                        break 'contact_loop;
                    }
                }
            }
        }
    }

    fn handle_death_zones(&mut self, ctx: &mut ScriptContext) {
        let graph = &ctx.scene.graph;

        let level = ctx.plugins.get::<Game>().level.as_ref().unwrap();
        for zone in level.death_zones.iter() {
            let zone_bounds = graph[*zone].world_bounding_box();
            let self_position = graph[ctx.handle].global_position();
            if zone_bounds.is_contains_point(self_position) {
                ctx.message_sender.send_hierarchical(
                    ctx.handle,
                    RoutingStrategy::Up,
                    HitBoxMessage::Damage(HitBoxDamage {
                        hit_box: ctx.handle,
                        damage: 10000.0,
                        dealer: DamageDealer::default(),
                        position: None,
                        is_melee: false,
                    }),
                );
            }
        }
    }

    fn on_damage(&mut self, damage: &HitBoxDamage, ctx: &mut ScriptMessageContext) {
        let prev_is_sliced_off = self.is_sliced_off();
        *self.health -= damage.damage;

        if let Some(position) = damage.position {
            if !prev_is_sliced_off && self.is_sliced_off() {
                if let Some(prefab) = self.destruction_prefab.as_ref() {
                    prefab.instantiate_at(
                        ctx.scene,
                        position.point,
                        vector_to_quat(position.direction),
                    );
                }
            }

            let prefab = if damage.is_melee {
                self.melee_hit_prefab.as_ref()
            } else {
                self.hit_prefab.as_ref()
            };
            if let Some(prefab) = prefab {
                prefab.instantiate_at(
                    ctx.scene,
                    position.point,
                    vector_to_quat(position.direction),
                );
            }

            if let Some(damage_prefab) = self.damage_prefab.as_ref() {
                damage_prefab.instantiate_and_attach(
                    ctx.scene,
                    ctx.handle,
                    position.point,
                    position.direction,
                    Vector3::repeat(1.0),
                );
            }

            if let Some(pierce_prefab) = self.pierce_prefab.as_ref() {
                let mut query_buffer = Vec::default();

                ctx.scene.graph.physics.cast_ray(
                    RayCastOptions {
                        ray_origin: Point3::from(position.point),
                        ray_direction: position.direction,
                        max_len: position.direction.norm(),
                        groups: Default::default(),
                        sort_results: true,
                    },
                    &mut query_buffer,
                );

                for intersection in query_buffer.iter() {
                    if matches!(
                        ctx.scene.graph[intersection.collider].as_collider().shape(),
                        ColliderShape::Trimesh(_)
                    ) && intersection
                        .position
                        .coords
                        .metric_distance(&position.point)
                        < 2.0
                    {
                        pierce_prefab.instantiate_and_attach(
                            ctx.scene,
                            intersection.collider,
                            intersection.position.coords,
                            position.direction,
                            Vector3::repeat(1.0),
                        );

                        break;
                    }
                }
            }
        }

        // Propagate the damage down.
        for child in self.children_hit_boxes.iter() {
            ctx.message_sender.send_to_target(
                *child,
                HitBoxMessage::Damage(HitBoxDamage {
                    hit_box: *child,
                    damage: damage.damage,
                    dealer: damage.dealer,
                    position: damage.position,
                    is_melee: damage.is_melee,
                }),
            );
        }
    }

    fn on_heal(&mut self, heal: &HitBoxHeal) {
        *self.health += heal.amount;
    }
}

impl ScriptTrait for HitBox {
    fn on_start(&mut self, ctx: &mut ScriptContext) {
        ctx.plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .unwrap()
            .hit_boxes
            .insert(ctx.handle);

        ctx.message_dispatcher
            .subscribe_to::<HitBoxMessage>(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        ctx.plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .unwrap()
            .hit_boxes
            .remove(&ctx.node_handle);
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        self.handle_death_zones(ctx);
        self.handle_environment_interaction(ctx);
        if self.is_sliced_off() {
            if let Some(bone) = ctx.scene.graph.try_get_mut(*self.bone) {
                bone.local_transform_mut().set_scale(Vector3::repeat(0.0));
            }
        }
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        let Some(hit_box_message) = message.downcast_ref::<HitBoxMessage>() else {
            return;
        };

        match hit_box_message {
            HitBoxMessage::Damage(damage) => self.on_damage(damage, ctx),
            HitBoxMessage::Heal(heal) => self.on_heal(heal),
        }
    }
}
