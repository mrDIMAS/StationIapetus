//! Small helper script that does a ray cast and scales the parent node with the distance
//! from the position of the node to the intersection point.

use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        math::ray::Ray,
        reflect::{FieldInfo, Reflect},
        type_traits::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    scene::{collider::InteractionGroups, graph::physics::RayCastOptions},
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "5405f6e2-3016-40ef-998a-e4f797e59694")]
#[visit(optional)]
pub struct Beam {
    max_length: f32,
}

impl Default for Beam {
    fn default() -> Self {
        Self { max_length: 100.0 }
    }
}

impl ScriptTrait for Beam {
    fn on_init(&mut self, context: &mut ScriptContext) {
        let node = &context.scene.graph[context.handle];
        let origin = node.global_position();
        let dir = node.look_vector();

        let physics = &mut context.scene.graph.physics;
        let ray = Ray::new(origin, dir);

        let mut query_buffer = Vec::default();

        physics.cast_ray(
            RayCastOptions {
                ray_origin: Point3::from(ray.origin),
                ray_direction: ray.dir,
                max_len: self.max_length,
                groups: InteractionGroups::default(),
                sort_results: true,
            },
            &mut query_buffer,
        );

        let len = query_buffer
            .first()
            .map_or(self.max_length, |i| i.toi.clamp(0.0, self.max_length));

        context.scene.graph[context.handle]
            .local_transform_mut()
            .set_scale(Vector3::new(1.0, 1.0, len));
    }
}
