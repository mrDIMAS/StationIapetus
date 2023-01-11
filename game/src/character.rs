use crate::{
    block_on,
    inventory::Inventory,
    level::item::{item_mut, ItemKind},
    sound::{SoundKind, SoundManager},
    weapon::{definition::WeaponKind, weapon_mut, weapon_ref},
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
    engine::resource_manager::ResourceManager,
    scene::{
        collider::Collider,
        graph::{map::NodeHandleMap, physics::RayCastOptions, Graph},
        node::Node,
        Scene,
    },
};
use std::collections::VecDeque;

pub enum CharacterMessage {
    BeganAiming(Handle<Node>),
    EndedAiming(Handle<Node>),
}

#[derive(Debug, Clone)]
pub enum CharacterCommand {
    SelectWeapon(WeaponKind),
    AddWeapon(WeaponKind),
    PickupItem(Handle<Node>),
    DropItems {
        item: ItemKind,
        count: u32,
    },
    Damage {
        /// Actor who damaged target actor, can be Handle::NONE if damage came from environment
        /// or not from any actor.
        who: Handle<Node>,
        /// A body part which was hit.
        hitbox: Option<HitBox>,
        /// Numeric value of damage.
        amount: f32,
        /// Only takes effect iff damage was applied to a head hit box!
        critical_shot_probability: f32,
    },
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
    pub inventory: Inventory,
    #[visit(skip)]
    #[reflect(hidden)]
    pub commands: VecDeque<CharacterCommand>,
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
            commands: Default::default(),
        }
    }
}

impl Character {
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
        for other_weapon in self.weapons.iter() {
            weapon_mut(*other_weapon, graph).enabled = false;
        }

        self.current_weapon = self.weapons.len() as u32;
        self.weapons.push(weapon);

        self.request_current_weapon_enabled(true, graph);
    }

    pub fn use_item(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Medkit => self.heal(40.0),
            ItemKind::Medpack => self.heal(20.0),
            // Non-consumable items.
            ItemKind::Ak47
            | ItemKind::PlasmaGun
            | ItemKind::M4
            | ItemKind::Glock
            | ItemKind::Ammo
            | ItemKind::RailGun
            | ItemKind::Grenade
            | ItemKind::MasterKey => (),
        }
    }

    pub fn push_command(&mut self, command: CharacterCommand) {
        self.commands.push_back(command);
    }

    pub fn poll_command(
        &mut self,
        scene: &mut Scene,
        self_handle: Handle<Node>,
        resource_manager: &ResourceManager,
        sound_manager: &SoundManager,
    ) -> Option<CharacterCommand> {
        if let Some(command) = self.commands.pop_front() {
            match command {
                CharacterCommand::SelectWeapon(kind) => self.select_weapon(kind, &mut scene.graph),
                CharacterCommand::AddWeapon(kind) => {
                    let weapon = block_on(
                        resource_manager.request_model(Weapon::definition(kind).model.clone()),
                    )
                    .unwrap()
                    .instantiate(scene);

                    // Root node must have Weapon script.
                    assert!(scene.graph[weapon].has_script::<Weapon>());

                    weapon_mut(weapon, &mut scene.graph).set_owner(self_handle);

                    self.add_weapon(weapon, &mut scene.graph);
                    scene.graph.link_nodes(weapon, self.weapon_pivot());
                    self.inventory_mut().add_item(kind.associated_item(), 1);
                }
                CharacterCommand::PickupItem(item_handle) => {
                    let position = scene.graph[item_handle].global_position();
                    let item = item_mut(item_handle, &mut scene.graph);

                    let kind = item.get_kind();

                    scene.graph.remove_node(item_handle);

                    sound_manager.play_sound(
                        &mut scene.graph,
                        "data/sounds/item_pickup.ogg",
                        position,
                        1.0,
                        3.0,
                        2.0,
                    );

                    match kind {
                        ItemKind::Medkit => self.inventory.add_item(ItemKind::Medkit, 1),
                        ItemKind::Medpack => self.inventory.add_item(ItemKind::Medpack, 1),
                        ItemKind::Ak47
                        | ItemKind::PlasmaGun
                        | ItemKind::M4
                        | ItemKind::Glock
                        | ItemKind::RailGun => {
                            let weapon_kind = kind.associated_weapon().unwrap();

                            let mut found = false;
                            for weapon_handle in self.weapons.iter() {
                                let weapon = weapon_ref(*weapon_handle, &scene.graph);
                                if weapon.kind() == weapon_kind {
                                    found = true;
                                    break;
                                }
                            }
                            if found {
                                self.inventory.add_item(ItemKind::Ammo, 24);
                            } else {
                                // Finally if actor does not have such weapon, give new one to him.
                                self.commands
                                    .push_back(CharacterCommand::AddWeapon(weapon_kind));
                            }
                        }
                        ItemKind::Ammo => {
                            self.inventory.add_item(ItemKind::Ammo, 24);
                        }
                        ItemKind::Grenade => {
                            self.inventory.add_item(ItemKind::Grenade, 1);
                        }
                        ItemKind::MasterKey => {
                            self.inventory.add_item(ItemKind::MasterKey, 1);
                        }
                    }
                }
                CharacterCommand::DropItems { item, count } => {
                    let drop_position = self.position(&scene.graph) + Vector3::new(0.0, 0.5, 0.0);
                    let weapons = self.weapons().to_vec();

                    if self.inventory.try_extract_exact_items(item, count) == count {
                        // Make sure to remove weapons associated with items.
                        if let Some(weapon_kind) = item.associated_weapon() {
                            for weapon in weapons {
                                if weapon_ref(weapon, &scene.graph).kind() == weapon_kind {
                                    scene.graph.remove_node(weapon);
                                }
                            }
                        }

                        Item::add_to_scene(
                            scene,
                            resource_manager.clone(),
                            item,
                            drop_position,
                            true,
                        );
                    }
                }
                CharacterCommand::Damage { amount, .. } => {
                    self.damage(amount);
                }
            }

            Some(command)
        } else {
            None
        }
    }

    pub fn select_weapon(&mut self, weapon: WeaponKind, graph: &mut Graph) {
        if let Some(index) = self
            .weapons
            .iter()
            .position(|&w| weapon_ref(w, graph).kind() == weapon)
        {
            for other_weapon in self.weapons.iter() {
                weapon_mut(*other_weapon, graph).enabled = false;
            }

            self.current_weapon = index as u32;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn current_weapon(&self) -> Handle<Node> {
        if let Some(weapon) = self.weapons.get(self.current_weapon as usize) {
            *weapon
        } else {
            Handle::NONE
        }
    }

    fn request_current_weapon_enabled(&self, state: bool, graph: &mut Graph) {
        if let Some(current_weapon) = self.weapons.get(self.current_weapon as usize) {
            weapon_mut(*current_weapon, graph).enabled = state;
        }
    }

    pub fn next_weapon(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() && (self.current_weapon as usize) < self.weapons.len() - 1 {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon += 1;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn prev_weapon(&mut self, graph: &mut Graph) {
        if self.current_weapon > 0 {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon -= 1;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn use_first_weapon_or_none(&mut self, graph: &mut Graph) {
        if !self.weapons.is_empty() {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon = 0;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn set_current_weapon(&mut self, i: usize, graph: &mut Graph) {
        if i < self.weapons.len() {
            self.request_current_weapon_enabled(false, graph);

            self.current_weapon = i as u32;

            self.request_current_weapon_enabled(true, graph);
        }
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        if scene.graph.is_valid_handle(self.body) {
            scene.graph.remove_node(self.body);
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
