use rg3d::{
    animation::{
        machine::{Machine, PoseNode, State},
        Animation,
    },
    asset::core::rand::Rng,
    core::{
        algebra::{Point3, Unit, UnitQuaternion, Vector3},
        pool::Handle,
    },
    engine::resource_manager::ResourceManager,
    rand,
    resource::{model::Model, texture::TextureWrapMode},
    scene::{
        base::BaseBuilder,
        camera::{CameraBuilder, SkyBoxBuilder},
        graph::Graph,
        node::Node,
        transform::TransformBuilder,
        Scene,
    },
    sound::{self, context::SoundContext},
};
use std::collections::HashMap;

pub mod model_map;

struct ImpactEntry {
    k: f32,
    source: UnitQuaternion<f32>,
}

#[derive(Default)]
pub struct BodyImpactHandler {
    additional_rotations: HashMap<Handle<Node>, ImpactEntry>,
}

impl BodyImpactHandler {
    pub fn handle_impact(
        &mut self,
        scene: &Scene,
        handle: Handle<Node>,
        impact_point: Vector3<f32>,
        direction: Vector3<f32>,
    ) {
        let global_transform = scene.graph[handle]
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
            .try_normalize(f32::EPSILON)
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
            let transform = scene.graph[*body].local_transform_mut();
            let new_rotation = **transform.rotation() * additional_rotation;
            transform.set_rotation(new_rotation);
        }
        self.additional_rotations.retain(|_, e| e.k < 1.0);
    }

    pub fn is_affected(&self, handle: Handle<Node>) -> bool {
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
    let (front, back, left, right, top, bottom) = rg3d::core::futures::join!(
        resource_manager.request_texture("data/textures/skyboxes/space/front.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/back.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/left.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/right.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/top.png"),
        resource_manager.request_texture("data/textures/skyboxes/space/bottom.png")
    );

    // Unwrap everything.
    let skybox = SkyBoxBuilder {
        front: Some(front.unwrap()),
        back: Some(back.unwrap()),
        left: Some(left.unwrap()),
        right: Some(right.unwrap()),
        top: Some(top.unwrap()),
        bottom: Some(bottom.unwrap()),
    }
    .build()
    .unwrap();

    // Set S and T coordinate wrap mode, ClampToEdge will remove any possible seams on edges
    // of the skybox.
    if let Some(skybox_texture) = skybox.cubemap() {
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

pub fn use_hrtf(context: SoundContext) {
    let hrtf_sphere = rg3d::sound::hrtf::HrirSphere::from_file(
        "data/sounds/hrtf.bin",
        sound::context::SAMPLE_RATE,
    )
    .unwrap();
    context
        .state()
        .set_renderer(rg3d::sound::renderer::Renderer::HrtfRenderer(
            rg3d::sound::renderer::hrtf::HrtfRenderer::new(hrtf_sphere),
        ));
}

pub fn create_play_animation_state(
    animation_resource: Model,
    name: &str,
    machine: &mut Machine,
    scene: &mut Scene,
    model: Handle<Node>,
) -> (Handle<Animation>, Handle<State>) {
    let animation = *animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    let node = machine.add_node(PoseNode::make_play_animation(animation));
    let state = machine.add_state(State::new(name, node));
    (animation, state)
}

pub fn is_probability_event_occurred(probability: f32) -> bool {
    return rand::thread_rng().gen_range(0.0..1.0) < probability.clamp(0.0, 1.0);
}
