use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        color_gradient::{ColorGradient, GradientPoint},
        pool::Handle,
    },
    engine::resource_manager::ResourceManager,
    scene::{
        base::BaseBuilder,
        graph::Graph,
        node::Node,
        particle_system::{particle::Particle, ParticleSystemBuilder},
        transform::TransformBuilder,
    },
};
use std::path::Path;

pub fn create_rail(
    graph: &mut Graph,
    resource_manager: &ResourceManager,
    begin: Vector3<f32>,
    end: Vector3<f32>,
    color: Color,
) -> Handle<Node> {
    let len = end.metric_distance(&begin);
    let total_particles = ((len * 150.0) as usize).min(20000);
    let radius = 0.02;

    ParticleSystemBuilder::new(
        BaseBuilder::new().with_local_transform(
            TransformBuilder::new()
                .with_local_position(begin)
                .with_local_rotation(UnitQuaternion::face_towards(&(end - begin), &Vector3::y()))
                .build(),
        ),
    )
    .with_acceleration(Vector3::new(0.0, 0.0, 0.0))
    .with_particles(
        (0..total_particles)
            .into_iter()
            .map(|i| {
                let t = i as f32 / total_particles as f32;

                let x = (t * len * 20.0).cos() * radius;
                let y = (t * len * 20.0).sin() * radius;
                let z = t * len;

                Particle::default()
                    .with_position(Vector3::new(x, y, z))
                    .with_size(0.01)
                    .with_initial_lifetime(3.0)
            })
            .collect::<Vec<_>>(),
    )
    .with_color_over_lifetime_gradient({
        let mut gradient = ColorGradient::new();
        gradient.add_point(GradientPoint::new(0.00, color.with_new_alpha(255)));
        gradient.add_point(GradientPoint::new(1.00, color.with_new_alpha(0)));
        gradient
    })
    .with_texture(resource_manager.request_texture(Path::new("data/particles/circle_05.png")))
    .build(graph)
}
