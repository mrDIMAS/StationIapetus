//! Particles generator for rail gun's rail effect.

use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        math::ray::Ray,
        reflect::{FieldInfo, Reflect},
        type_traits::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    scene::{
        collider::InteractionGroups,
        graph::physics::RayCastOptions,
        particle_system::{particle::Particle, ParticleSystem},
    },
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "bdefd227-b1eb-4f8e-9ef9-8a8ec4abab1c")]
#[visit(optional)]
pub struct Rail {
    radius: f32,
    particles_per_meter: f32,
    max_length: f32,
}

impl Default for Rail {
    fn default() -> Self {
        Self {
            radius: 0.1,
            particles_per_meter: 120.0,
            max_length: 100.0,
        }
    }
}

impl ScriptTrait for Rail {
    fn on_init(&mut self, context: &mut ScriptContext) {
        let node = &context.scene.graph[context.handle];
        let origin = node.global_position();
        let dir = node.look_vector();

        // Do a ray-cast from the position of the node first.
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

        let total_particles = ((len * self.particles_per_meter) as usize).min(20000);

        if let Some(particle_system) = context
            .scene
            .graph
            .try_get_mut_of_type::<ParticleSystem>(context.handle)
        {
            particle_system.set_particles(
                (0..total_particles)
                    .map(|i| {
                        let t = i as f32 / total_particles as f32;

                        let x = (t * len * 20.0).cos() * self.radius;
                        let y = (t * len * 20.0).sin() * self.radius;
                        let z = t * len;

                        Particle::default()
                            .with_position(Vector3::new(x, y, z))
                            .with_size(0.01)
                            .with_initial_lifetime(3.0)
                    })
                    .collect::<Vec<_>>(),
            );
        }
    }
}
