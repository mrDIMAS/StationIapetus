use fyrox::graph::{SceneGraph, SceneGraphNode};
use fyrox::{
    core::{
        algebra::{Matrix4, Vector3},
        math::aabb::AxisAlignedBoundingBox,
        reflect::prelude::*,
        type_traits::prelude::*,
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    scene::rigidbody::RigidBody,
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "d5a6d420-bb6c-4367-ad06-26109880eff8")]
#[visit(optional)]
pub struct Explosion {
    strength: InheritableVariable<f32>,
    scale: InheritableVariable<Vector3<f32>>,
}

impl Default for Explosion {
    fn default() -> Self {
        Self {
            strength: 100.0f32.into(),
            scale: Vector3::new(2.0, 2.0, 2.0).into(),
        }
    }
}

impl ScriptTrait for Explosion {
    fn on_start(&mut self, context: &mut ScriptContext) {
        let node = &context.scene.graph[context.handle];
        let aabb = AxisAlignedBoundingBox::unit()
            .transform(&(node.global_transform() * Matrix4::new_nonuniform_scaling(&*self.scale)));
        let center = aabb.center();
        for rigid_body in context
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
    }
}
