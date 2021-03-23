use rg3d::core::pool::Handle;
use rg3d::engine::resource_manager::ResourceManager;
use rg3d::resource::texture::TextureWrapMode;
use rg3d::scene::base::BaseBuilder;
use rg3d::scene::camera::{CameraBuilder, SkyBox};
use rg3d::scene::graph::Graph;
use rg3d::scene::node::Node;
use rg3d::scene::transform::TransformBuilder;
use rg3d::{
    core::algebra::{Point3, Unit, UnitQuaternion, Vector3},
    scene::{RigidBodyHandle, Scene},
};
use std::collections::HashMap;

struct ImpactEntry {
    k: f32,
    source: UnitQuaternion<f32>,
}

#[derive(Default)]
pub struct BodyImpactHandler {
    additional_rotations: HashMap<RigidBodyHandle, ImpactEntry>,
}

impl BodyImpactHandler {
    pub fn handle_impact(
        &mut self,
        scene: &Scene,
        handle: RigidBodyHandle,
        impact_point: Vector3<f32>,
        direction: Vector3<f32>,
    ) {
        let global_transform = scene.graph[scene.physics_binder.node_of(handle).unwrap()]
            .global_transform()
            .try_inverse()
            .unwrap_or_default();
        let local_impact_point = global_transform.transform_point(&Point3::from(impact_point));
        let local_direction = global_transform.transform_vector(&direction);
        // local_impact_point can be directly be used as vector because it is in
        // local coordinates of rigid body.
        if let Some(axis) = local_impact_point
            .coords
            .cross(&local_direction)
            .try_normalize(std::f32::EPSILON)
        {
            let additional_rotation =
                UnitQuaternion::from_axis_angle(&Unit::new_normalize(axis), 24.0f32.to_radians());
            self.additional_rotations
                .entry(handle)
                .and_modify(|r| {
                    r.source = additional_rotation;
                    r.k = 0.0;
                })
                .or_insert(ImpactEntry {
                    k: 0.0,
                    source: additional_rotation,
                });
        }
    }

    pub fn update_and_apply(&mut self, dt: f32, scene: &mut Scene) {
        for (body, entry) in self.additional_rotations.iter_mut() {
            let additional_rotation = entry.source.nlerp(&UnitQuaternion::default(), entry.k);
            entry.k += dt;
            let node = scene.physics_binder.node_of(*body).unwrap();
            let transform = scene.graph[node].local_transform_mut();
            let new_rotation = **transform.rotation() * additional_rotation;
            transform.set_rotation(new_rotation);
        }
        self.additional_rotations.retain(|_, e| e.k < 1.0);
    }

    pub fn is_affected(&self, handle: RigidBodyHandle) -> bool {
        self.additional_rotations.contains_key(&handle)
    }
}

/// Creates a camera at given position with a skybox.
pub async fn create_camera(
    resource_manager: ResourceManager,
    position: Vector3<f32>,
    graph: &mut Graph,
    z_far: f32,
) -> Handle<Node> {
    // Load skybox textures in parallel.
    let (front, back, left, right, top, bottom) = rg3d::futures::join!(
        resource_manager.request_texture("data/textures/skyboxes/space/front.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/back.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/left.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/right.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/top.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/bottom.png")
    );

    // Unwrap everything.
    let skybox = SkyBox {
        front: Some(front.unwrap()),
        back: Some(back.unwrap()),
        left: Some(left.unwrap()),
        right: Some(right.unwrap()),
        top: Some(top.unwrap()),
        bottom: Some(bottom.unwrap()),
    };

    // Set S and T coordinate wrap mode, ClampToEdge will remove any possible seams on edges
    // of the skybox.
    for skybox_texture in skybox.textures().iter().filter_map(|t| t.clone()) {
        let mut data = skybox_texture.data_ref();
        data.set_s_wrap_mode(TextureWrapMode::ClampToEdge);
        data.set_t_wrap_mode(TextureWrapMode::ClampToEdge);
    }

    // Camera is our eyes in the world - you won't see anything without it.
    CameraBuilder::new(
        BaseBuilder::new().with_local_transform(
            TransformBuilder::new()
                .with_local_position(position)
                .build(),
        ),
    )
    .with_z_far(z_far)
    .with_skybox(skybox)
    .build(graph)
}
