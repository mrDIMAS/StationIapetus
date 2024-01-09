use crate::{bot::Bot, door::Door, level::Level};
use fyrox::{
    core::{
        color::Color, math::aabb::AxisAlignedBoundingBox, pool::Handle, reflect::prelude::*,
        type_traits::prelude::*, variable::InheritableVariable, visitor::prelude::*,
    },
    scene::{debug::SceneDrawingContext, graph::Graph, node::Node, Scene},
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone)]
enum State {
    #[default]
    Inactive,
    Active,
    Finished,
}

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "845a5364-395a-4228-9394-ee3c43352f01")]
pub struct EnemyTrap {
    #[visit(optional)]
    doors_to_lock: InheritableVariable<Vec<Handle<Node>>>,

    #[visit(optional)]
    nodes_to_enable_on_activation: InheritableVariable<Vec<Handle<Node>>>,

    #[visit(optional)]
    nodes_to_enable_on_deactivation: InheritableVariable<Vec<Handle<Node>>>,

    #[visit(optional)]
    #[reflect(hidden)]
    enemies: Vec<Handle<Node>>,

    #[visit(optional)]
    #[reflect(hidden)]
    state: State,
}

impl EnemyTrap {
    fn find_enemies(
        &mut self,
        scene: &Scene,
        actors: &[Handle<Node>],
        this_bounds: &AxisAlignedBoundingBox,
    ) {
        for actor in actors {
            if let Some(actor_node) = scene.graph.try_get(*actor) {
                if this_bounds.is_contains_point(actor_node.global_position())
                    && actor_node.try_get_script_component::<Bot>().is_some()
                {
                    self.enemies.push(*actor);
                }
            }
        }
    }

    fn is_all_enemies_dead(&self, scene: &Scene) -> bool {
        for enemy in self.enemies.iter() {
            if let Some(actor_node) = scene.graph.try_get(*enemy) {
                if let Some(bot) = actor_node.try_get_script_component::<Bot>() {
                    if !bot.is_dead() {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn lock_doors(&mut self, scene: &mut Scene, lock: bool) {
        for door in self.doors_to_lock.iter() {
            if let Some(door) = scene.graph[*door].try_get_script_component_mut::<Door>() {
                door.locked.set_value_and_mark_modified(lock);
            }
        }
    }

    pub fn editor_debug_draw(&self, node: &Node, drawing_context: &mut SceneDrawingContext) {
        drawing_context.draw_aabb(
            &AxisAlignedBoundingBox::unit().transform(&node.global_transform()),
            Color::RED,
        );
    }

    fn enable_nodes(&self, graph: &mut Graph, animations: &[Handle<Node>]) {
        for node_handle in animations.iter() {
            if let Some(node) = graph.try_get_mut(*node_handle) {
                node.set_enabled(true);
            }
        }
    }
}

impl ScriptTrait for EnemyTrap {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        for animation in self
            .nodes_to_enable_on_activation
            .iter()
            .chain(&*self.nodes_to_enable_on_deactivation)
        {
            ctx.scene.graph[*animation].set_enabled(false);
        }
    }

    fn on_update(&mut self, context: &mut ScriptContext) {
        match self.state {
            State::Inactive => {
                if let Some(level) = Level::try_get(context.plugins) {
                    let this = &context.scene.graph[context.handle];
                    let this_bounds =
                        AxisAlignedBoundingBox::unit().transform(&this.global_transform());

                    let player_position = context.scene.graph[level.player].global_position();

                    if this_bounds.is_contains_point(player_position) {
                        self.state = State::Active;

                        self.find_enemies(context.scene, &level.actors, &this_bounds);
                        self.lock_doors(context.scene, true);
                        self.enable_nodes(
                            &mut context.scene.graph,
                            &self.nodes_to_enable_on_activation,
                        );
                    }
                }
            }
            State::Active => {
                if self.is_all_enemies_dead(context.scene) {
                    self.lock_doors(context.scene, false);
                    self.enable_nodes(
                        &mut context.scene.graph,
                        &self.nodes_to_enable_on_deactivation,
                    );
                    self.state = State::Finished;
                }
            }
            State::Finished => {
                // Do nothing.
            }
        }
    }
}
