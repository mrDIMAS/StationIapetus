use fyrox::{
    core::{
        algebra::Vector3,
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

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct Explosion {
    strength: InheritableVariable<f32>,
}

impl_component_provider!(Explosion);

impl TypeUuidProvider for Explosion {
    fn type_uuid() -> Uuid {
        uuid!("d5a6d420-bb6c-4367-ad06-26109880eff8")
    }
}

impl ScriptTrait for Explosion {
    fn on_start(&mut self, context: &mut ScriptContext) {
        context
            .scene
            .graph
            .update_hierarchical_data_for_descendants(context.handle);
        let node = &context.scene.graph[context.handle];
        let aabb = node.world_bounding_box();
        let center = aabb.center();
        for rigid_body in context
            .scene
            .graph
            .linear_iter_mut()
            .filter_map(|n| n.query_component_mut::<RigidBody>())
        {
            let d = rigid_body.global_position() - center;
            let k = d.component_div(&aabb.half_extents());
            let force = (Vector3::repeat(1.0) - k.inf(&Vector3::repeat(1.0))).scale(*self.strength);
            rigid_body.apply_force(force);
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
