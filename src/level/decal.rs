use rg3d::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        math::vector_to_quat,
        pool::Handle,
        visitor::prelude::*,
        VecExtensions,
    },
    engine::resource_manager::ResourceManager,
    scene::{
        base::BaseBuilder, decal::DecalBuilder, graph::Graph, node::Node,
        transform::TransformBuilder,
    },
};

#[derive(Default, Visit)]
pub struct Decal {
    node: Handle<Node>,
    lifetime: f32,
    fade_interval: f32,
}

impl Decal {
    pub fn new(node: Handle<Node>, lifetime: f32, fade_interval: f32) -> Self {
        Self {
            node,
            lifetime,
            fade_interval,
        }
    }

    pub fn new_shot_impact(
        resource_manager: ResourceManager,
        graph: &mut Graph,
        position: Vector3<f32>,
        face_towards: Vector3<f32>,
    ) -> Self {
        Self {
            node: DecalBuilder::new(
                BaseBuilder::new().with_local_transform(
                    TransformBuilder::new()
                        .with_local_position(position)
                        .with_local_rotation(
                            vector_to_quat(face_towards)
                                * UnitQuaternion::from_axis_angle(
                                    &Vector3::x_axis(),
                                    90.0f32.to_radians(),
                                ),
                        )
                        .with_local_scale(Vector3::new(0.05, 0.05, 0.05))
                        .build(),
                ),
            )
            .with_diffuse_texture(
                resource_manager.request_texture("data/textures/decals/BulletImpact_BaseColor.png"),
            )
            .build(graph),
            lifetime: 10.0,
            fade_interval: 1.0,
        }
    }
}

#[derive(Default, Visit)]
pub struct DecalContainer {
    decals: Vec<Decal>,
}

impl DecalContainer {
    pub fn add(&mut self, decal: Decal) {
        self.decals.push(decal);
    }

    pub fn update(&mut self, graph: &mut Graph, dt: f32) {
        self.decals.retain_mut(|decal| {
            decal.lifetime -= dt;

            let abs_lifetime = decal.lifetime.abs();

            let alpha = if decal.lifetime <= 0.0 {
                1.0 - (abs_lifetime / decal.fade_interval).min(1.0)
            } else {
                1.0
            };

            let decal_node = graph[decal.node].as_decal_mut();

            decal_node.set_color(Color::from_rgba(255, 255, 255, (255.0 * alpha) as u8));

            if decal.lifetime < 0.0 && abs_lifetime > decal.fade_interval {
                graph.remove_node(decal.node);

                false
            } else {
                true
            }
        });
    }
}
