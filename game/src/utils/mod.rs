use fyrox::graph::SceneGraph;
use fyrox::plugin::error::GameResult;
use fyrox::scene::sound::Status;
use fyrox::{
    asset::{core::rand::Rng, manager::ResourceManager},
    core::{
        algebra::{Point3, Unit, UnitQuaternion, Vector3},
        pool::Handle,
    },
    rand::{self, seq::IteratorRandom},
    scene::{
        animation::prelude::*,
        graph::Graph,
        node::Node,
        sound::{context::SoundContext, HrirSphereResourceData, Sound},
        Scene,
    },
};
use std::{collections::HashMap, fmt::Debug};

pub mod model_map;

#[derive(Clone, Debug)]
struct ImpactEntry {
    k: f32,
    source: UnitQuaternion<f32>,
}

#[derive(Default, Debug, Clone)]
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
    ) -> GameResult {
        let node = scene.graph.try_get(handle)?;
        let global_transform = node.global_transform().try_inverse().unwrap_or_default();
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
        Ok(())
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

pub async fn use_hrtf(context: &mut SoundContext, resource_manager: &ResourceManager) {
    let hrtf_sphere = resource_manager
        .request::<HrirSphereResourceData>("data/sounds/hrtf.hrir")
        .await
        .unwrap();

    context
        .state()
        .set_renderer(fyrox::scene::sound::Renderer::HrtfRenderer(
            fyrox::scene::sound::HrtfRenderer::new(hrtf_sphere),
        ));
}

pub fn is_probability_event_occurred(probability: f32) -> bool {
    rand::thread_rng().gen_range(0.0..1.0) < probability.clamp(0.0, 1.0)
}

pub fn fetch_animation_container_ref(
    graph: &Graph,
    handle: Handle<AnimationPlayer>,
) -> &AnimationContainer {
    graph.try_get(handle).unwrap().animations()
}

pub fn fetch_animation_container_mut(
    graph: &mut Graph,
    handle: Handle<AnimationPlayer>,
) -> &mut AnimationContainer {
    graph
        .try_get_mut(handle)
        .unwrap()
        .animations_mut()
        .get_value_mut_silent()
}

pub fn is_any_sound_playing(sounds: &[Handle<Node>], graph: &Graph) -> bool {
    sounds.iter().any(|h| {
        graph
            .try_get_of_type::<Sound>(*h)
            .ok()
            .is_some_and(|s| s.status() == Status::Playing)
    })
}

pub fn try_play_random_sound(sounds: &[Handle<Node>], graph: &mut Graph) -> bool {
    if let Some(random_sound) = sounds
        .iter()
        .choose(&mut rand::thread_rng())
        .and_then(|s| graph.try_get_mut_of_type::<Sound>(*s).ok())
    {
        random_sound.play();
        true
    } else {
        false
    }
}

pub fn try_play_sound(sound_handle: Handle<Node>, graph: &mut Graph) -> GameResult {
    graph.try_get_mut_of_type::<Sound>(sound_handle)?.try_play();
    Ok(())
}
