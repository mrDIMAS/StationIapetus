use crate::message::Message;
use rg3d::core::sstorage::ImmutableString;
use rg3d::material::PropertyValue;
use rg3d::{
    core::{algebra::Vector3, pool::Handle, visitor::prelude::*},
    engine::resource_manager::ResourceManager,
    physics3d::{rapier::geometry::FeatureId, ColliderHandle},
    rand::{self, seq::SliceRandom},
    scene::{node::Node, Scene},
    sound::{
        context::SoundContext,
        effects::{BaseEffect, Effect, EffectInput},
        source::{generic::GenericSourceBuilder, spatial::SpatialSourceBuilder, Status},
    },
    utils::log::{Log, MessageKind},
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, ops::Range, path::Path, path::PathBuf, time::Duration};

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
    material_to_sound: HashMap<MaterialType, HashMap<SoundKind, Vec<PathBuf>>>,
    texture_to_material: HashMap<PathBuf, MaterialType>,
}

impl SoundBase {
    pub fn load() -> Self {
        let file = File::open("data/sounds/sound_map.ron").unwrap();
        let mut base: Self = ron::de::from_reader(file).unwrap();
        // Canonicalize paths to remove \ and / differences and remove prefixes like ./ etc.
        base.texture_to_material = base
            .texture_to_material
            .iter()
            .filter_map(|(path, material_type)| match path.canonicalize() {
                Ok(canonicalized) => Some((canonicalized, material_type.clone())),
                Err(e) => {
                    Log::writeln(
                        MessageKind::Error,
                        format!(
                            "[Sound Manager]: Failed to \
                                            canonicalize path {}! Reason: {}",
                            path.display(),
                            e
                        ),
                    );

                    None
                }
            })
            .collect::<HashMap<_, _>>();
        base
    }
}

#[derive(Default)]
pub struct SoundMap {
    sound_map: HashMap<ColliderHandle, Vec<TriangleRange>>,
}

impl SoundMap {
    pub fn new(scene: &Scene, sound_base: &SoundBase) -> Self {
        let mut sound_map = HashMap::new();

        let mut nodes_with_physics = Vec::new();
        for (handle, _) in scene.graph.pair_iter() {
            if scene.physics_binder.body_of(handle).is_some() {
                nodes_with_physics.push(handle)
            }
        }

        let mut stack = Vec::new();

        for node in nodes_with_physics {
            let body = scene.physics_binder.body_of(node).unwrap();

            if let Some(body) = scene.physics.bodies.get(body) {
                if let Some(&collider) = body.colliders().first() {
                    let collider = scene
                        .physics
                        .colliders
                        .handle_map()
                        .key_of(&collider)
                        .cloned()
                        .unwrap();

                    let mut ranges = Vec::new();

                    stack.clear();
                    stack.push(node);

                    let mut triangle_offset = 0u32;
                    while let Some(handle) = stack.pop() {
                        let descendant = &scene.graph[handle];

                        if let Node::Mesh(descendant_mesh) = descendant {
                            for surface in descendant_mesh.surfaces() {
                                let data = surface.data();
                                let data = data.lock();

                                if let Some(diffuse_texture) = surface
                                    .material()
                                    .lock()
                                    .property_ref(&ImmutableString::new("diffuseTexture"))
                                {
                                    if let PropertyValue::Sampler {
                                        value: Some(diffuse_texture),
                                        ..
                                    } = diffuse_texture
                                    {
                                        let state = diffuse_texture.state();
                                        match state.path().canonicalize() {
                                            Ok(path) => {
                                                if let Some(&material) =
                                                    sound_base.texture_to_material.get(&*path)
                                                {
                                                    ranges.push(TriangleRange {
                                                        range: triangle_offset
                                                            ..(triangle_offset
                                                                + data.geometry_buffer.len()
                                                                    as u32),
                                                        material,
                                                    });
                                                } else {
                                                    Log::writeln(
                                                        MessageKind::Warning,
                                                        format!(
                                                            "[Sound Manager]: A texture {} does not have \
                                        respective mapping in sound map! \
                                        Environment sounds (footsteps, impact, etc.) \
                                        won't play for this texture!",
                                                            path.display()
                                                        ),
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                Log::writeln(
                                                    MessageKind::Error,
                                                    format!(
                                                        "[Sound Manager]: Failed to \
                                            canonicalize path {}! Reason: {}",
                                                        state.path().display(),
                                                        e
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }

                                triangle_offset += data.geometry_buffer.len() as u32;
                            }
                        }

                        stack.extend_from_slice(descendant.children());
                    }

                    sound_map.insert(collider, ranges);
                }
            }
        }
        Self { sound_map }
    }

    pub fn ranges_of(&self, collider: ColliderHandle) -> Option<&[TriangleRange]> {
        self.sound_map.get(&collider).map(|r| r.as_slice())
    }
}

#[derive(Default, Visit)]
pub struct SoundManager {
    context: SoundContext,
    reverb: Handle<Effect>,
    #[visit(skip)]
    sound_base: SoundBase,
    #[visit(skip)]
    sound_map: SoundMap,
}

impl SoundManager {
    pub fn new(context: SoundContext, scene: &Scene) -> Self {
        let mut base_effect = BaseEffect::default();
        base_effect.set_gain(0.7);
        let mut reverb = rg3d::sound::effects::reverb::Reverb::new(base_effect);
        reverb.set_dry(0.5);
        reverb.set_wet(0.5);
        reverb.set_decay_time(Duration::from_secs_f32(3.0));
        let reverb = context
            .state()
            .add_effect(rg3d::sound::effects::Effect::Reverb(reverb));

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
        if let Ok(buffer) = resource_manager.request_sound_buffer(path, false).await {
            let shot_sound = SpatialSourceBuilder::new(
                GenericSourceBuilder::new()
                    .with_buffer(buffer.into())
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

            let mut state = self.context.state();
            let source = state.add_source(shot_sound);
            state
                .effect_mut(self.reverb)
                .add_input(EffectInput::direct(source));
        } else {
            Log::writeln(
                MessageKind::Error,
                format!("Unable to play sound {:?}", path),
            );
        }
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
                gain,
                rolloff_factor,
                radius,
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
                            if let Some(sound) = sound_list.choose(&mut rand::thread_rng()) {
                                self.play_sound(
                                    sound.as_ref(),
                                    position,
                                    gain,
                                    rolloff_factor,
                                    radius,
                                    resource_manager,
                                )
                                .await;
                            }
                        } else {
                            Log::writeln(
                                MessageKind::Warning,
                                format!(
                                    "Unable to play environment sound: there \
                                is no respective mapping for {:?} sound kind!",
                                    sound_kind
                                ),
                            );
                        }
                    } else {
                        Log::writeln(
                            MessageKind::Warning,
                            format!(
                                "Unable to play environment sound: there \
                                is no respective mapping for {:?} material!",
                                material
                            ),
                        );
                    }
                } else {
                    Log::writeln(
                        MessageKind::Warning,
                        "Unable to play environment sound: unable to fetch material type!"
                            .to_owned(),
                    );
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
