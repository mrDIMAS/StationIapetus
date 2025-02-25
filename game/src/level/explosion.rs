use crate::level::hit_box::HitBoxDamage;
use crate::{
    character::{DamageDealer, DamagePosition},
    level::hit_box::HitBoxMessage,
    Game,
};
use fyrox::{
    core::{
        algebra::{Matrix4, Vector3},
        math::aabb::AxisAlignedBoundingBox,
        reflect::prelude::*,
        type_traits::prelude::*,
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    graph::{SceneGraph, SceneGraphNode},
    scene::rigidbody::RigidBody,
    script::{RoutingStrategy, ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "d5a6d420-bb6c-4367-ad06-26109880eff8")]
#[visit(optional)]
pub struct Explosion {
    strength: InheritableVariable<f32>,
    scale: InheritableVariable<Vector3<f32>>,
    damage: InheritableVariable<Option<f32>>,
}

impl Default for Explosion {
    fn default() -> Self {
        Self {
            strength: 100.0f32.into(),
            scale: Vector3::new(2.0, 2.0, 2.0).into(),
            damage: Default::default(),
        }
    }
}

impl ScriptTrait for Explosion {
    fn on_start(&mut self, ctx: &mut ScriptContext) {
        let node = &ctx.scene.graph[ctx.handle];
        let aabb = AxisAlignedBoundingBox::unit()
            .transform(&(node.global_transform() * Matrix4::new_nonuniform_scaling(&*self.scale)));
        let center = aabb.center();
        for rigid_body in ctx
            .scene
            .graph
            .linear_iter_mut()
            .filter_map(|n| n.component_mut::<RigidBody>())
        {
            if aabb.is_contains_point(rigid_body.global_position()) {
                let d = rigid_body.global_position() - center;
                let force = d.normalize().scale(*self.strength);
                rigid_body.apply_force(force);
                rigid_body.wake_up();
            }
        }

        if let Some(damage) = *self.damage {
            let game = ctx.plugins.get::<Game>();
            let level = game.level.as_ref().unwrap();

            for &hit_box in level.hit_boxes.iter() {
                let hit_box_ref = &ctx.scene.graph[hit_box];
                let position = hit_box_ref.global_position();
                let direction = hit_box_ref.global_position() - center;
                if aabb.is_contains_point(position) {
                    ctx.message_sender.send_hierarchical(
                        hit_box,
                        RoutingStrategy::Up,
                        HitBoxMessage::Damage(HitBoxDamage {
                            hit_box,
                            damage,
                            dealer: DamageDealer::default(),
                            position: Some(DamagePosition {
                                point: position,
                                direction,
                            }),
                            is_melee: false,
                        }),
                    );
                }
            }
        }
    }
}
