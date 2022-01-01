use rg3d::core::algebra::Point3;
use rg3d::resource::texture::Texture;
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
    decal: Handle<Node>,
    lifetime: f32,
    fade_interval: f32,
}

impl Decal {
    pub fn new(
        graph: &mut Graph,
        position: Vector3<f32>,
        face_towards: Vector3<f32>,
        parent: Handle<Node>,
        color: Color,
        scale: Vector3<f32>,
        texture: Texture,
    ) -> Self {
        let (position, face_towards, scale) = if parent.is_some() {
            let parent_scale = graph.global_scale(parent);

            let parent_inv_transform = graph[parent]
                .global_transform()
                .try_inverse()
                .unwrap_or_default();

            (
                parent_inv_transform
                    .transform_point(&Point3::from(position))
                    .coords,
                parent_inv_transform.transform_vector(&face_towards),
                // Discard parent's scale.
                Vector3::new(
                    scale.x / parent_scale.x,
                    scale.y / parent_scale.y,
                    scale.z / parent_scale.z,
                ),
            )
        } else {
            (position, face_towards, scale)
        };

        let rotation = vector_to_quat(face_towards)
            * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 90.0f32.to_radians());

        let decal = DecalBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(position)
                    .with_local_rotation(rotation)
                    .with_local_scale(scale)
                    .build(),
            ),
        )
        .with_diffuse_texture(texture)
        .with_color(color)
        .build(graph);

        if decal.is_some() && parent.is_some() {
            graph.link_nodes(decal, parent);
        }

        Self {
            decal,
            lifetime: 10.0,
            fade_interval: 1.0,
        }
    }

    pub fn new_bullet_hole(
        resource_manager: ResourceManager,
        graph: &mut Graph,
        position: Vector3<f32>,
        face_towards: Vector3<f32>,
        parent: Handle<Node>,
        color: Color,
    ) -> Self {
        let default_scale = Vector3::new(0.05, 0.05, 0.05);

        Self::new(
            graph,
            position,
            face_towards,
            parent,
            color,
            default_scale,
            resource_manager.request_texture("data/textures/decals/BulletImpact_BaseColor.png"),
        )
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
        self.decals.retain_mut_ext(|decal| {
            decal.lifetime -= dt;

            let abs_lifetime = decal.lifetime.abs();

            let alpha = if decal.lifetime <= 0.0 {
                1.0 - (abs_lifetime / decal.fade_interval).min(1.0)
            } else {
                1.0
            };

            let decal_node = graph[decal.decal].as_decal_mut();

            decal_node.set_color(decal_node.color().with_new_alpha((255.0 * alpha) as u8));

            if decal.lifetime < 0.0 && abs_lifetime > decal.fade_interval {
                graph.remove_node(decal.decal);

                false
            } else {
                true
            }
        });
    }
}
