use crate::{
    block_on,
    inventory::Inventory,
    level::item::ItemAction,
    sound::{SoundKind, SoundManager},
    weapon::{weapon_mut, WeaponMessage, WeaponMessageData},
    Item, Weapon,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        math::ray::Ray,
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    resource::model::{ModelResource, ModelResourceExtension},
    scene::{
        collider::Collider,
        graph::{map::NodeHandleMap, physics::RayCastOptions, Graph},
        node::Node,
        Scene,
    },
    script::ScriptMessageSender,
};
use std::ops::Deref;

#[derive(Copy, Clone)]
pub struct DamageDealer {
    pub entity: Handle<Node>,
}

impl DamageDealer {
    pub fn as_character<'a>(&self, graph: &'a Graph) -> Option<(Handle<Node>, &'a Character)> {
        if let Some(dealer_script) = graph.try_get(self.entity).and_then(|n| n.script()) {
            if let Some(character) = dealer_script.query_component_ref::<Character>() {
                return Some((self.entity, character));
            } else if let Some(weapon) = dealer_script.query_component_ref::<Weapon>() {
                if let Some(weapon_owner_script) =
                    graph.try_get(weapon.owner()).and_then(|n| n.script())
                {
                    if let Some(character_owner) =
                        weapon_owner_script.query_component_ref::<Character>()
                    {
                        return Some((weapon.owner(), character_owner));
                    }
                }
            }
        }
        None
    }
}

#[derive(Clone, Copy)]
pub struct DamagePosition {
    pub point: Vector3<f32>,
    pub direction: Vector3<f32>,
}

pub enum CharacterMessageData {
    BeganAiming,
    EndedAiming,
    Damage {
        /// An entity which damaged the target actor. It could be a handle of a character that done a melee attack,
        /// or a weapon that made a ray hit, or a projectile (such as grenade).
        dealer: DamageDealer,
        /// A body part which was hit.
        hitbox: Option<HitBox>,
        /// Numeric value of damage.
        amount: f32,
        /// Only takes effect iff damage was applied to a head hit box!
        critical_hit_probability: f32,
        position: Option<DamagePosition>,
    },
    SelectWeapon(ModelResource),
    AddWeapon(ModelResource),
    PickupItem(Handle<Node>),
    DropItems {
        item: ModelResource,
        count: u32,
    },
}

pub struct CharacterMessage {
    pub character: Handle<Node>,
    pub data: CharacterMessageData,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Character {
    pub capsule_collider: Handle<Node>,
    pub body: Handle<Node>,
    pub health: f32,
    pub last_health: f32,
    pub weapons: Vec<Handle<Node>>,
    pub current_weapon: u32,
    pub weapon_pivot: Handle<Node>,
    #[visit(optional)]
    pub hit_boxes: Vec<HitBox>,
    #[visit(optional)]
    pub inventory: Inventory,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            capsule_collider: Default::default(),
            body: Default::default(),
            health: 100.0,
            last_health: 100.0,
            weapons: Vec::new(),
            current_weapon: 0,
            weapon_pivot: Handle::NONE,
            hit_boxes: Default::default(),
            inventory: Default::default(),
        }
    }
}

impl Character {
    pub fn stand_still(&self, graph: &mut Graph) {
        let body = graph[self.body].as_rigid_body_mut();
        body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));
    }

    pub fn has_ground_contact(&self, graph: &Graph) -> bool {
        if let Some(collider) = graph
            .try_get(self.capsule_collider)
            .and_then(|n| n.cast::<Collider>())
        {
            for contact in collider.contacts(&graph.physics) {
                for manifold in contact.manifolds.iter() {
                    if manifold.local_n1.y.abs() > 0.7 || manifold.local_n2.y.abs() > 0.7 {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn get_health(&self) -> f32 {
        self.health
    }

    pub fn set_position(&mut self, graph: &mut Graph, position: Vector3<f32>) {
        if let Some(body) = graph.try_get_mut(self.body) {
            body.local_transform_mut().set_position(position);
        }
    }

    pub fn position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.body].global_position()
    }

    pub fn damage(&mut self, amount: f32) {
        self.health -= amount.abs();
    }

    pub fn heal(&mut self, amount: f32) {
        self.health += amount.abs();

        if self.health > 150.0 {
            self.health = 150.0;
        }
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    pub fn weapon_pivot(&self) -> Handle<Node> {
        self.weapon_pivot
    }

    pub fn weapons(&self) -> &[Handle<Node>] {
        &self.weapons
    }

    pub fn add_weapon(&mut self, weapon: Handle<Node>, graph: &mut Graph) {
        for &other_weapon in self.weapons.iter() {
            graph[other_weapon].set_enabled(false);
        }

        self.current_weapon = self.weapons.len() as u32;
        self.weapons.push(weapon);

        self.set_current_weapon_enabled(true, graph);
    }

    pub fn use_item(&mut self, item: &Item) {
        match *item.action {
            ItemAction::None => {}
            ItemAction::Heal { amount } => {
                self.heal(amount);
            }
        }
    }

    pub fn on_weapon_message(&mut self, weapon_message: &WeaponMessage, graph: &mut Graph) {
        if let WeaponMessageData::Removed = weapon_message.data {
            let removed_weapon = weapon_message.weapon;
            let current_weapon = self.current_weapon();

            if let Some(i) = self.weapons.iter().position(|&w| w == removed_weapon) {
                self.weapons.remove(i);
            }

            if current_weapon == removed_weapon && !self.weapons.is_empty() {
                self.current_weapon = 0;
                self.set_current_weapon_enabled(true, graph);
            }
        }
    }

    pub fn on_character_message(
        &mut self,
        message_data: &CharacterMessageData,
        scene: &mut Scene,
        self_handle: Handle<Node>,
        script_message_sender: &ScriptMessageSender,
        sound_manager: &SoundManager,
    ) {
        match message_data {
            CharacterMessageData::Damage { amount, .. } => {
                self.damage(*amount);
            }
            CharacterMessageData::SelectWeapon(weapon_resource) => {
                self.select_weapon(weapon_resource.clone(), &mut scene.graph)
            }
            CharacterMessageData::AddWeapon(weapon_resource) => {
                let weapon = block_on(weapon_resource.clone())
                    .unwrap()
                    .instantiate(scene);

                // Root node must have Weapon script.
                assert!(scene.graph[weapon].has_script::<Weapon>());

                let weapon_script = weapon_mut(weapon, &mut scene.graph);

                weapon_script.set_owner(self_handle);

                if let Some(associated_item) = weapon_script.associated_item.as_ref() {
                    self.inventory_mut().add_item(associated_item, 1);
                }

                self.add_weapon(weapon, &mut scene.graph);
                scene.graph.link_nodes(weapon, self.weapon_pivot());
            }
            &CharacterMessageData::PickupItem(item_handle) => {
                let item_node = &scene.graph[item_handle];
                let item_resource = item_node.root_resource();
                let item = item_node.try_get_script::<Item>().unwrap();
                let stack_size = *item.stack_size;
                let position = item_node.global_position();

                if let Some(associated_weapon) = item.associated_weapon.deref().clone() {
                    let mut found = false;
                    for weapon_handle in self.weapons.iter() {
                        if scene.graph[*weapon_handle].root_resource()
                            == Some(associated_weapon.clone())
                        {
                            found = true;
                            break;
                        }
                    }
                    if found {
                        Weapon::from_resource(&associated_weapon, |weapon| {
                            if let Some(associated_weapon) = weapon {
                                if let Some(ammo_item) = associated_weapon.ammo_item.as_ref() {
                                    self.inventory.add_item(ammo_item, 24);
                                }
                            }
                        });
                    } else {
                        // Finally if actor does not have such weapon, give new one to him.
                        script_message_sender.send_to_target(
                            self_handle,
                            CharacterMessage {
                                character: self_handle,
                                data: CharacterMessageData::AddWeapon(associated_weapon.clone()),
                            },
                        );
                    }
                }

                if let Some(item_resource) = item_resource {
                    self.inventory.add_item(&item_resource, stack_size);
                }

                sound_manager.play_sound(
                    &mut scene.graph,
                    "data/sounds/item_pickup.ogg",
                    position,
                    1.0,
                    3.0,
                    2.0,
                );

                scene.graph.remove_node(item_handle);
            }
            CharacterMessageData::DropItems { item, count } => {
                let drop_position = self.position(&scene.graph) + Vector3::new(0.0, 0.5, 0.0);
                let weapons = self.weapons().to_vec();

                if self.inventory.try_extract_exact_items(&item, *count) == *count {
                    // Make sure to remove weapons associated with items.
                    Item::from_resource(&item, |item| {
                        if let Some(item) = item {
                            if let Some(weapon_resource) = item.associated_weapon.as_ref() {
                                for &weapon in weapons.iter() {
                                    if scene.graph[weapon].root_resource()
                                        == Some(weapon_resource.clone())
                                    {
                                        scene.graph.remove_node(weapon);
                                    }
                                }
                            }
                        }
                    });

                    Item::add_to_scene(scene, item.clone(), drop_position, true, *count);
                }
            }
            _ => (),
        }
    }

    pub fn select_weapon(&mut self, weapon: ModelResource, graph: &mut Graph) {
        if let Some(index) = self
            .weapons
            .iter()
            .position(|&w| graph[w].root_resource() == Some(weapon.clone()))
        {
            for &other_weapon in self.weapons.iter() {
                graph[other_weapon].set_enabled(false);
            }

            self.current_weapon = index as u32;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn current_weapon(&self) -> Handle<Node> {
        self.weapons
            .get(self.current_weapon as usize)
            .cloned()
            .unwrap_or_default()
    }

    fn set_current_weapon_enabled(&self, state: bool, graph: &mut Graph) {
        if let Some(current_weapon) = self.weapons.get(self.current_weapon as usize) {
            graph[*current_weapon].set_enabled(state);
        }
    }

    pub fn next_weapon(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() && (self.current_weapon as usize) < self.weapons.len() - 1 {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon += 1;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn prev_weapon(&mut self, graph: &mut Graph) {
        if self.current_weapon > 0 {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon -= 1;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn use_first_weapon_or_none(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon = 0;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn set_current_weapon(&mut self, i: usize, graph: &mut Graph) {
        if i < self.weapons.len() {
            self.set_current_weapon_enabled(false, graph);

            self.current_weapon = i as u32;

            self.set_current_weapon_enabled(true, graph);
        }
    }

    pub fn inventory(&self) -> &Inventory {
        &self.inventory
    }

    pub fn inventory_mut(&mut self) -> &mut Inventory {
        &mut self.inventory
    }

    pub fn footstep_ray_check(
        &self,
        begin: Vector3<f32>,
        scene: &mut Scene,
        manager: &SoundManager,
    ) {
        let mut query_buffer = Vec::new();

        let ray = Ray::from_two_points(begin, begin + Vector3::new(0.0, -100.0, 0.0));

        scene.graph.physics.cast_ray(
            RayCastOptions {
                ray_origin: Point3::from(ray.origin),
                ray_direction: ray.dir,
                max_len: 100.0,
                groups: Default::default(),
                sort_results: true,
            },
            &mut query_buffer,
        );

        for intersection in query_buffer
            .into_iter()
            .filter(|i| i.collider != self.capsule_collider)
        {
            manager.play_environment_sound(
                &mut scene.graph,
                intersection.collider,
                intersection.feature,
                intersection.position.coords,
                SoundKind::FootStep,
                0.2,
                1.0,
                0.3,
            );
        }
    }
}

#[derive(Default, Clone, Copy, PartialEq, Debug, Visit, Reflect)]
pub struct HitBox {
    pub bone: Handle<Node>,
    pub collider: Handle<Node>,
    pub damage_factor: f32,
    pub movement_speed_factor: f32,
    pub is_head: bool,
}

impl HitBox {
    pub fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping.map(&mut self.collider);
    }
}

pub fn try_get_character_ref(handle: Handle<Node>, graph: &Graph) -> Option<&Character> {
    graph.try_get(handle).and_then(|c| {
        c.script()
            .and_then(|s| s.query_component_ref::<Character>())
    })
}

pub fn character_ref(handle: Handle<Node>, graph: &Graph) -> &Character {
    try_get_character_ref(handle, graph).unwrap()
}

pub fn try_get_character_mut(handle: Handle<Node>, graph: &mut Graph) -> Option<&mut Character> {
    graph.try_get_mut(handle).and_then(|c| {
        c.script_mut()
            .and_then(|s| s.query_component_mut::<Character>())
    })
}

pub fn character_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Character {
    try_get_character_mut(handle, graph).unwrap()
}
