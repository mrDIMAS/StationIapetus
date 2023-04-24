//! Weapon related stuff.

use crate::{
    utils::ResourceProxy,
    weapon::{
        definition::{WeaponDefinition, WeaponKind},
        projectile::Projectile,
    },
};
use fyrox::resource::model::ModelResourceExtension;
use fyrox::{
    core::{
        algebra::{Matrix3, Vector2, Vector3},
        math::{vector_to_quat, Matrix4Ext},
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    rand::{seq::SliceRandom, Rng},
    resource::model::ModelResource,
    scene::{graph::Graph, node::Node, Scene},
    script::{
        ScriptContext, ScriptDeinitContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait,
    },
};

pub mod definition;
pub mod projectile;
pub mod sight;

pub struct WeaponMessage {
    pub weapon: Handle<Node>,
    pub data: WeaponMessageData,
}

pub enum WeaponMessageData {
    Shoot { direction: Option<Vector3<f32>> },
    Removed,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Weapon {
    kind: WeaponKind,
    shot_point: Handle<Node>,
    flash_light: Handle<Node>,
    flash_light_enabled: bool,

    #[visit(optional)]
    shoot_interval: InheritableVariable<f32>,

    #[visit(optional)]
    pub yaw_correction: InheritableVariable<f32>,

    #[visit(optional)]
    pub pitch_correction: InheritableVariable<f32>,

    #[visit(optional)]
    pub ammo_indicator_offset: InheritableVariable<Vector3<f32>>,

    #[visit(optional)]
    pub ammo_consumption_per_shot: InheritableVariable<u32>,

    #[visit(optional)]
    pub v_recoil: InheritableVariable<Vector2<f32>>,

    #[visit(optional)]
    pub h_recoil: InheritableVariable<Vector2<f32>>,

    #[visit(optional)]
    projectile: Option<ModelResource>,

    #[visit(optional)]
    #[reflect(
        description = "A list of VFX resources that will be randomly instantiated on shot. Usually it is some sort of muzzle flash."
    )]
    shot_vfx: InheritableVariable<Vec<ResourceProxy<ModelResource>>>,

    #[reflect(hidden)]
    owner: Handle<Node>,

    #[reflect(hidden)]
    #[visit(optional)]
    last_shot_time: f32,

    #[reflect(hidden)]
    #[visit(skip)]
    pub definition: &'static WeaponDefinition,

    #[reflect(hidden)]
    #[visit(skip)]
    self_handle: Handle<Node>,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            kind: WeaponKind::M4,
            shot_point: Handle::NONE,
            last_shot_time: 0.0,
            owner: Handle::NONE,
            definition: Self::definition(WeaponKind::M4),
            flash_light: Default::default(),
            flash_light_enabled: false,
            shoot_interval: 0.15.into(),
            projectile: None,
            self_handle: Default::default(),
            yaw_correction: (-4.0).into(),
            pitch_correction: (-12.0).into(),
            ammo_indicator_offset: Vector3::new(-0.09, 0.03, 0.0).into(),
            ammo_consumption_per_shot: 2.into(),
            v_recoil: Vector2::new(-2.0, 4.0).into(),
            h_recoil: Vector2::new(-1.0, 1.0).into(),
            shot_vfx: Default::default(),
        }
    }
}

impl Weapon {
    pub fn definition(kind: WeaponKind) -> &'static WeaponDefinition {
        definition::DEFINITIONS.map.get(&kind).unwrap()
    }

    pub fn shot_position(&self, graph: &Graph) -> Vector3<f32> {
        if self.shot_point.is_some() {
            graph[self.shot_point].global_position()
        } else {
            // Fallback
            graph[self.self_handle].global_position()
        }
    }

    pub fn shot_direction(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.self_handle].look_vector().normalize()
    }

    pub fn kind(&self) -> WeaponKind {
        self.kind
    }

    pub fn world_basis(&self, graph: &Graph) -> Matrix3<f32> {
        graph[self.self_handle].global_transform().basis()
    }

    pub fn owner(&self) -> Handle<Node> {
        self.owner
    }

    pub fn set_owner(&mut self, owner: Handle<Node>) {
        self.owner = owner;
    }

    pub fn switch_flash_light(&mut self) {
        self.flash_light_enabled = !self.flash_light_enabled;
    }

    pub fn can_shoot(&self, elapsed_time: f32) -> bool {
        elapsed_time - self.last_shot_time >= *self.shoot_interval
    }

    pub fn gen_v_recoil_angle(&self) -> f32 {
        fyrox::rand::thread_rng()
            .gen_range(self.v_recoil.x.to_radians()..self.v_recoil.y.to_radians())
    }

    pub fn gen_h_recoil_angle(&self) -> f32 {
        fyrox::rand::thread_rng()
            .gen_range(self.h_recoil.x.to_radians()..self.h_recoil.y.to_radians())
    }

    fn shoot(
        &mut self,
        self_handle: Handle<Node>,
        scene: &mut Scene,
        elapsed_time: f32,
        direction: Option<Vector3<f32>>,
    ) {
        self.last_shot_time = elapsed_time;

        let shot_position = self.shot_position(&scene.graph);
        let direction = direction
            .unwrap_or_else(|| self.shot_direction(&scene.graph))
            .try_normalize(f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        if let Some(vfx) = self
            .shot_vfx
            .choose(&mut fyrox::rand::thread_rng())
            .and_then(|vfx| vfx.0.as_ref())
        {
            vfx.instantiate_at(scene, shot_position, vector_to_quat(direction));
        }

        if let Some(model) = self.projectile.as_ref() {
            Projectile::spawn(
                model,
                scene,
                direction,
                shot_position,
                self_handle,
                Default::default(),
            );
        }
    }
}

impl_component_provider!(Weapon);

impl TypeUuidProvider for Weapon {
    fn type_uuid() -> Uuid {
        uuid!("bca0083b-b062-4d95-b241-db05bca65da7")
    }
}

impl ScriptTrait for Weapon {
    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.definition = Self::definition(self.kind);
        self.self_handle = ctx.handle;

        ctx.message_dispatcher
            .subscribe_to::<WeaponMessage>(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        ctx.message_sender.send_global(WeaponMessage {
            weapon: ctx.node_handle,
            data: WeaponMessageData::Removed,
        });
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        if let Some(flash_light) = ctx.scene.graph.try_get_mut(self.flash_light) {
            flash_light.set_visibility(self.flash_light_enabled);
        }
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        if let Some(msg) = message.downcast_ref::<WeaponMessage>() {
            if msg.weapon != ctx.handle {
                return;
            }

            if let WeaponMessageData::Shoot { direction } = msg.data {
                self.shoot(ctx.handle, ctx.scene, ctx.elapsed_time, direction);
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

pub fn try_weapon_mut(handle: Handle<Node>, graph: &mut Graph) -> Option<&mut Weapon> {
    graph
        .try_get_mut(handle)
        .and_then(|node| node.try_get_script_mut::<Weapon>())
}

pub fn weapon_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Weapon {
    graph[handle].try_get_script_mut::<Weapon>().unwrap()
}

pub fn try_weapon_ref(handle: Handle<Node>, graph: &Graph) -> Option<&Weapon> {
    graph
        .try_get(handle)
        .and_then(|node| node.try_get_script::<Weapon>())
}

pub fn weapon_ref(handle: Handle<Node>, graph: &Graph) -> &Weapon {
    try_weapon_ref(handle, graph).unwrap()
}
