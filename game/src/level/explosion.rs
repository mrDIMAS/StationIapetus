use fyrox::{
    core::{
        algebra::{Matrix4, Vector3},
        math::aabb::AxisAlignedBoundingBox,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    scene::rigidbody::RigidBody,
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Explosion {
    #[visit(optional)]
    strength: InheritableVariable<f32>,
    #[visit(optional)]
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

impl_component_provider!(Explosion);

impl TypeUuidProvider for Explosion {
    fn type_uuid() -> Uuid {
        uuid!("d5a6d420-bb6c-4367-ad06-26109880eff8")
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
            .filter_map(|n| n.query_component_mut::<RigidBody>())
        {
            if aabb.is_contains_point(rigid_body.global_position()) {
                let d = rigid_body.global_position() - center;
                let k = (Vector3::repeat(1.0) - d.component_div(&aabb.half_extents()))
                    .scale(*self.strength);
                let force = d.normalize().component_mul(&k);
                rigid_body.apply_force(force);
                rigid_body.wake_up();
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
