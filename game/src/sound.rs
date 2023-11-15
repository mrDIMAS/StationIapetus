use fyrox::{
    asset::manager::ResourceManager,
    core::{
        algebra::Vector3,
        futures::executor::block_on,
        log::{Log, MessageKind},
        pool::Handle,
        sstorage::ImmutableString,
    },
    material::PropertyValue,
    rand::{self, seq::SliceRandom},
    scene::{
        base::BaseBuilder,
        graph::{physics::FeatureId, Graph},
        mesh::Mesh,
        node::Node,
        sound::{reverb::Reverb, Effect, SoundBuffer, SoundBufferResource, SoundBuilder, Status},
        transform::TransformBuilder,
        Scene,
    },
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, ops::Range, path::Path, path::PathBuf};

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
                Ok(canonicalized) => Some((canonicalized, *material_type)),
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
    sound_map: HashMap<Handle<Node>, Vec<TriangleRange>>,
}

impl SoundMap {
    pub fn new(scene: &Scene, sound_base: &SoundBase) -> Self {
        let mut sound_map = HashMap::new();

        let mut stack = Vec::new();

        for (node, body) in scene.graph.pair_iter().filter(|(_, n)| n.is_rigid_body()) {
            for &collider in body.children() {
                if scene.graph[collider].is_collider() {
                    let mut ranges = Vec::new();

                    stack.clear();
                    stack.push(node);

                    let mut triangle_offset = 0u32;
                    while let Some(handle) = stack.pop() {
                        let descendant = &scene.graph[handle];

                        if let Some(descendant_mesh) = descendant.cast::<Mesh>() {
                            for surface in descendant_mesh.surfaces() {
                                let data = surface.data();
                                let data = data.lock();

                                if let Some(PropertyValue::Sampler {
                                    value: Some(diffuse_texture),
                                    ..
                                }) = surface
                                    .material()
                                    .data_ref()
                                    .property_ref(&ImmutableString::new("diffuseTexture"))
                                {
                                    let path = diffuse_texture.path();
                                    match path.canonicalize() {
                                        Ok(path) => {
                                            if let Some(&material) =
                                                sound_base.texture_to_material.get(&*path)
                                            {
                                                ranges.push(TriangleRange {
                                                    range: triangle_offset
                                                        ..(triangle_offset
                                                            + data.geometry_buffer.len() as u32),
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
                                                    path.display(),
                                                    e
                                                ),
                                            );
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

    pub fn ranges_of(&self, collider: Handle<Node>) -> Option<&[TriangleRange]> {
        self.sound_map.get(&collider).map(|r| r.as_slice())
    }
}

#[derive(Default)]
pub struct SoundManager {
    sound_base: SoundBase,
    sound_map: SoundMap,
    resource_manager: Option<ResourceManager>,
}

impl SoundManager {
    pub fn new(scene: &mut Scene, resource_manager: ResourceManager) -> Self {
        let mut reverb = Reverb::new();
        reverb.set_dry(0.5);
        reverb.set_wet(0.5);
        reverb.set_decay_time(3.0);
        scene
            .graph
            .sound_context
            .state()
            .bus_graph_mut()
            .primary_bus_mut()
            .add_effect(Effect::Reverb(reverb));

        let sound_base = SoundBase::load();

        Self {
            sound_map: SoundMap::new(scene, &sound_base),
            sound_base,
            resource_manager: Some(resource_manager),
        }
    }

    pub fn try_play_sound_buffer(
        &self,
        graph: &mut Graph,
        buffer: Option<&SoundBufferResource>,
        position: Vector3<f32>,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    ) {
        if let Some(buffer) = buffer {
            self.play_sound_buffer(graph, buffer, position, gain, rolloff_factor, radius)
        } else {
            Log::warn("Failed to play a sound!")
        }
    }

    pub fn play_sound_buffer(
        &self,
        graph: &mut Graph,
        buffer: &SoundBufferResource,
        position: Vector3<f32>,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    ) {
        SoundBuilder::new(
            BaseBuilder::new().with_local_transform(
                TransformBuilder::new()
                    .with_local_position(position)
                    .build(),
            ),
        )
        .with_buffer(buffer.clone().into())
        .with_status(Status::Playing)
        .with_play_once(true)
        .with_gain(gain)
        .with_radius(radius)
        .with_rolloff_factor(rolloff_factor)
        .build(graph);
    }

    pub fn play_sound<P: AsRef<Path>>(
        &self,
        graph: &mut Graph,
        path: P,
        position: Vector3<f32>,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    ) {
        if let Ok(buffer) = block_on(
            self.resource_manager
                .as_ref()
                .unwrap()
                .request::<SoundBuffer, _>(path.as_ref()),
        ) {
            self.play_sound_buffer(graph, &buffer, position, gain, rolloff_factor, radius)
        } else {
            Log::writeln(
                MessageKind::Error,
                format!("Unable to play sound {:?}", path.as_ref()),
            );
        }
    }

    pub fn play_environment_sound(
        &self,
        graph: &mut Graph,
        collider: Handle<Node>,
        feature: FeatureId,
        position: Vector3<f32>,
        sound_kind: SoundKind,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    ) {
        let material = self.sound_map.ranges_of(collider).and_then(|ranges| {
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
        });

        if let Some(material) = material {
            if let Some(map) = self.sound_base.material_to_sound.get(&material) {
                if let Some(sound_list) = map.get(&sound_kind) {
                    if let Some(sound) = sound_list.choose(&mut rand::thread_rng()) {
                        self.play_sound(graph, sound, position, gain, rolloff_factor, radius);
                    }
                } else {
                    Log::writeln(
                        MessageKind::Warning,
                        format!(
                            "Unable to play environment sound: there \
                                is no respective mapping for {sound_kind:?} sound kind!"
                        ),
                    );
                }
            } else {
                Log::writeln(
                    MessageKind::Warning,
                    format!(
                        "Unable to play environment sound: there \
                                is no respective mapping for {material:?} material!"
                    ),
                );
            }
        } else {
            Log::warn("Unable to play environment sound: unable to fetch material type!");
        }
    }
}
