use crate::message::Message;
use rg3d::{
    core::{
        algebra::Vector3,
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    physics::geometry::FeatureId,
    rand,
    rand::seq::SliceRandom,
    scene::{node::Node, ColliderHandle, Scene},
    sound::{
        context::{self, Context},
        effects::{BaseEffect, Effect, EffectInput},
        source::{generic::GenericSourceBuilder, spatial::SpatialSourceBuilder, Status},
    },
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, ops::Range, path::Path, time::Duration};

#[derive(Debug)]
pub struct TriangleRange {
    range: Range<u32>,
    material: MaterialType,
}

#[derive(Deserialize, Hash, Eq, PartialEq, Copy, Clone, Debug)]
pub enum MaterialType {
    Grass,
    Metal,
    Stone,
    Wood,
    Chain,
    Flesh,
}

#[derive(Deserialize, Hash, Eq, PartialEq, Copy, Clone, Debug)]
pub enum SoundKind {
    Impact,
    FootStep,
}

#[derive(Deserialize, Debug, Default)]
pub struct SoundBase {
    material_to_sound: HashMap<MaterialType, HashMap<SoundKind, Vec<String>>>,
    texture_to_material: HashMap<String, MaterialType>,
}

impl SoundBase {
    pub fn load() -> Self {
        let file = File::open("data/sounds/sound_map.ron").unwrap();
        ron::de::from_reader(file).unwrap()
    }
}

#[derive(Default)]
pub struct SoundMap {
    sound_map: HashMap<ColliderHandle, Vec<TriangleRange>>,
}

impl SoundMap {
    pub fn new(scene: &Scene, sound_base: &SoundBase) -> Self {
        let mut sound_map = HashMap::new();

        let mut nodes = Vec::new();
        for (handle, _) in scene.graph.pair_iter() {
            if scene.physics_binder.body_of(handle).is_some() {
                nodes.push(handle)
            }
        }

        let mut stack = Vec::new();

        for node in nodes {
            let body = scene.physics_binder.body_of(node).unwrap();

            if let Some(&collider) = scene
                .physics
                .bodies
                .get(body.into())
                .unwrap()
                .colliders()
                .first()
            {
                let mut ranges = Vec::new();

                stack.clear();
                stack.push(node);

                let mut triangle_offset = 0u32;
                while let Some(handle) = stack.pop() {
                    let descendant = &scene.graph[handle];

                    if let Node::Mesh(descendant_mesh) = descendant {
                        for surface in descendant_mesh.surfaces() {
                            let data = surface.data();
                            let data = data.read().unwrap();

                            if let Some(diffuse_texture) = surface.diffuse_texture() {
                                let path =
                                    diffuse_texture.state().path().to_string_lossy().to_string();
                                if let Some(&material) = sound_base.texture_to_material.get(&path) {
                                    ranges.push(TriangleRange {
                                        range: triangle_offset
                                            ..(triangle_offset + data.triangles().len() as u32),
                                        material,
                                    });
                                }
                            }
                            triangle_offset += data.triangles().len() as u32;
                        }
                    }

                    stack.extend_from_slice(descendant.children());
                }

                sound_map.insert(collider.into(), ranges);
            }
        }
        Self { sound_map }
    }

    pub fn ranges_of(&self, collider: ColliderHandle) -> Option<&[TriangleRange]> {
        self.sound_map.get(&collider).map(|r| r.as_slice())
    }
}

#[derive(Default)]
pub struct SoundManager {
    context: Context,
    reverb: Handle<Effect>,
    sound_base: SoundBase,
    sound_map: SoundMap,
}

impl SoundManager {
    pub fn new(context: Context, scene: &Scene) -> Self {
        let mut base_effect = BaseEffect::default();
        base_effect.set_gain(0.7);
        let mut reverb = rg3d::sound::effects::reverb::Reverb::new(base_effect);
        reverb.set_dry(0.5);
        reverb.set_wet(0.5);
        reverb.set_decay_time(Duration::from_secs_f32(3.0));
        let reverb = context
            .state()
            .add_effect(rg3d::sound::effects::Effect::Reverb(reverb));

        let hrtf_sphere = rg3d::sound::hrtf::HrirSphere::from_file(
            "data/sounds/IRC_1040_C.bin",
            context::SAMPLE_RATE,
        )
        .unwrap();
        context
            .state()
            .set_renderer(rg3d::sound::renderer::Renderer::HrtfRenderer(
                rg3d::sound::renderer::hrtf::HrtfRenderer::new(hrtf_sphere),
            ));

        let sound_base = SoundBase::load();

        Self {
            context,
            reverb,
            sound_map: SoundMap::new(scene, &sound_base),
            sound_base,
        }
    }

    async fn play_sound(
        &self,
        path: &Path,
        position: Vector3<f32>,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
        resource_manager: ResourceManager,
    ) {
        let mut state = self.context.state();
        let buffer = resource_manager
            .request_sound_buffer(path, false)
            .await
            .unwrap();
        let shot_sound = SpatialSourceBuilder::new(
            GenericSourceBuilder::new(buffer.into())
                .with_status(Status::Playing)
                .with_play_once(true)
                .with_gain(gain)
                .build()
                .unwrap(),
        )
        .with_position(position)
        .with_radius(radius)
        .with_rolloff_factor(rolloff_factor)
        .build_source();
        let source = state.add_source(shot_sound);
        state
            .effect_mut(self.reverb)
            .add_input(EffectInput::direct(source));
    }

    pub async fn handle_message(&mut self, resource_manager: ResourceManager, message: &Message) {
        match message {
            Message::PlaySound {
                path,
                position,
                gain,
                rolloff_factor,
                radius,
            } => {
                self.play_sound(
                    path,
                    *position,
                    *gain,
                    *rolloff_factor,
                    *radius,
                    resource_manager,
                )
                .await;
            }
            &Message::PlayEnvironmentSound {
                collider,
                feature,
                position,
                sound_kind,
            } => {
                let material = self
                    .sound_map
                    .ranges_of(collider)
                    .map(|ranges| {
                        match feature {
                            FeatureId::Face(idx) => {
                                let mut material = None;
                                for range in ranges {
                                    if range.range.contains(&idx) {
                                        material = Some(range.material);
                                        break;
                                    }
                                }
                                material
                            }
                            _ => {
                                // Some object have convex shape colliders, they're not provide any
                                // useful info about the point of impact, so we have to use first
                                // available material.
                                ranges.first().map(|first_range| first_range.material)
                            }
                        }
                    })
                    .flatten();

                if let Some(material) = material {
                    if let Some(map) = self.sound_base.material_to_sound.get(&material) {
                        if let Some(sound_list) = map.get(&sound_kind) {
                            self.play_sound(
                                sound_list.choose(&mut rand::thread_rng()).unwrap().as_ref(),
                                position,
                                1.0,
                                1.0,
                                10.0,
                                resource_manager,
                            )
                            .await;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn resolve(&mut self, scene: &Scene) {
        self.sound_base = SoundBase::load();
        self.sound_map = SoundMap::new(scene, &self.sound_base);
    }
}

impl Visit for SoundManager {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.context.visit("Context", visitor)?;
        self.reverb.visit("Reverb", visitor)?;

        visitor.leave_region()
    }
}
