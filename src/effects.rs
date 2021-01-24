use rg3d::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        color_gradient::{ColorGradient, GradientPoint},
        numeric_range::NumericRange,
        pool::Handle,
    },
    engine::resource_manager::ResourceManager,
    scene::{
        base::BaseBuilder,
        graph::Graph,
        node::Node,
        particle_system::{BaseEmitterBuilder, ParticleSystemBuilder, SphereEmitterBuilder},
        transform::TransformBuilder,
    },
};
use std::path::Path;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum EffectKind {
    BulletImpact,
    Smoke,
}

pub fn create(
    kind: EffectKind,
    graph: &mut Graph,
    resource_manager: ResourceManager,
    pos: Vector3<f32>,
    orientation: UnitQuaternion<f32>,
) -> Handle<Node> {
    match kind {
        EffectKind::BulletImpact => create_bullet_impact(graph, resource_manager, pos, orientation),
        EffectKind::Smoke => create_smoke(graph, resource_manager, pos, orientation),
    }
}

fn create_bullet_impact(
    graph: &mut Graph,
    resource_manager: ResourceManager,
    pos: Vector3<f32>,
    orientation: UnitQuaternion<f32>,
) -> Handle<Node> {
    ParticleSystemBuilder::new(
        BaseBuilder::new().with_lifetime(1.0).with_local_transform(
            TransformBuilder::new()
                .with_local_position(pos)
                .with_local_rotation(orientation)
                .build(),
        ),
    )
    .with_acceleration(Vector3::new(0.0, -10.0, 0.0))
    .with_color_over_lifetime_gradient({
        let mut gradient = ColorGradient::new();
        gradient.add_point(GradientPoint::new(0.00, Color::from_rgba(255, 255, 0, 0)));
        gradient.add_point(GradientPoint::new(0.05, Color::from_rgba(255, 160, 0, 255)));
        gradient.add_point(GradientPoint::new(0.95, Color::from_rgba(255, 120, 0, 255)));
        gradient.add_point(GradientPoint::new(1.00, Color::from_rgba(255, 60, 0, 0)));
        gradient
    })
    .with_emitters(vec![SphereEmitterBuilder::new(
        BaseEmitterBuilder::new()
            .with_max_particles(200)
            .with_spawn_rate(1000)
            .with_size_modifier_range(NumericRange::new(-0.01, -0.0125))
            .with_size_range(NumericRange::new(0.015, 0.03))
            .with_x_velocity_range(NumericRange::new(-0.015, 0.015))
            .with_y_velocity_range(NumericRange::new(0.0175, 0.025))
            .with_z_velocity_range(NumericRange::new(-0.015, 0.015))
            .resurrect_particles(false),
    )
    .with_radius(0.01)
    .build()])
    .with_texture(resource_manager.request_texture(Path::new("data/particles/circle_05.png")))
    .build(graph)
}

fn create_smoke(
    graph: &mut Graph,
    resource_manager: ResourceManager,
    pos: Vector3<f32>,
    orientation: UnitQuaternion<f32>,
) -> Handle<Node> {
    ParticleSystemBuilder::new(
        BaseBuilder::new().with_lifetime(5.0).with_local_transform(
            TransformBuilder::new()
                .with_local_position(pos)
                .with_local_rotation(orientation)
                .build(),
        ),
    )
    .with_acceleration(Vector3::new(0.0, 0.0, 0.0))
    .with_color_over_lifetime_gradient({
        let mut gradient = ColorGradient::new();
        gradient.add_point(GradientPoint::new(0.00, Color::from_rgba(150, 150, 150, 0)));
        gradient.add_point(GradientPoint::new(
            0.05,
            Color::from_rgba(150, 150, 150, 220),
        ));
        gradient.add_point(GradientPoint::new(
            0.85,
            Color::from_rgba(255, 255, 255, 180),
        ));
        gradient.add_point(GradientPoint::new(1.00, Color::from_rgba(255, 255, 255, 0)));
        gradient
    })
    .with_emitters(vec![SphereEmitterBuilder::new(
        BaseEmitterBuilder::new()
            .with_max_particles(100)
            .with_spawn_rate(50)
            .with_x_velocity_range(NumericRange::new(-0.01, 0.01))
            .with_y_velocity_range(NumericRange::new(0.02, 0.03))
            .with_z_velocity_range(NumericRange::new(-0.01, 0.01)),
    )
    .with_radius(0.01)
    .build()])
    .with_texture(resource_manager.request_texture(Path::new("data/particles/smoke_04.tga")))
    .build(graph)
}
