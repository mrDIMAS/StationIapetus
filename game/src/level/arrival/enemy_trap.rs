use crate::{bot::Bot, door::Door, Game};
use fyrox::graph::SceneGraph;
use fyrox::plugin::error::{GameError, GameResult};
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
#[visit(optional)]
pub struct EnemyTrap {
    doors_to_lock: InheritableVariable<Vec<Handle<Node>>>,
    nodes_to_enable_on_activation: InheritableVariable<Vec<Handle<Node>>>,
    nodes_to_enable_on_deactivation: InheritableVariable<Vec<Handle<Node>>>,
    #[reflect(hidden)]
    enemies: Vec<Handle<Node>>,
    #[reflect(hidden)]
    state: State,
}

impl EnemyTrap {
    fn find_enemies(
        &mut self,
        scene: &Scene,
        actors: &[Handle<Node>],
        this_bounds: &AxisAlignedBoundingBox,
    ) -> GameResult {
        for actor in actors {
            let actor_node = scene.graph.try_get(*actor)?;
            if this_bounds.is_contains_point(actor_node.global_position())
                && actor_node.try_get_script_component::<Bot>().is_some()
            {
                self.enemies.push(*actor);
            }
        }
        Ok(())
    }

    fn is_all_enemies_dead(&self, scene: &Scene) -> Result<bool, GameError> {
        for enemy in self.enemies.iter() {
            let actor_node = scene.graph.try_get(*enemy)?;
            if let Some(bot) = actor_node.try_get_script_component::<Bot>() {
                if !bot.is_dead(&scene.graph) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
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

    fn enable_nodes(&self, graph: &mut Graph, animations: &[Handle<Node>]) -> GameResult {
        for node_handle in animations.iter() {
            graph.try_get_mut(*node_handle)?.set_enabled(true);
        }
        Ok(())
    }
}

impl ScriptTrait for EnemyTrap {
    fn on_init(&mut self, ctx: &mut ScriptContext) -> GameResult {
        for animation in self
            .nodes_to_enable_on_activation
            .iter()
            .chain(&*self.nodes_to_enable_on_deactivation)
        {
            ctx.scene.graph[*animation].set_enabled(false);
        }
        Ok(())
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) -> GameResult {
        match self.state {
            State::Inactive => {
                if let Some(level) = ctx.plugins.get::<Game>().level.as_ref() {
                    let this = &ctx.scene.graph[ctx.handle];
                    let this_bounds =
                        AxisAlignedBoundingBox::unit().transform(&this.global_transform());

                    let player_position = ctx.scene.graph.try_get(level.player)?.global_position();

                    if this_bounds.is_contains_point(player_position) {
                        self.state = State::Active;

                        self.find_enemies(ctx.scene, &level.actors, &this_bounds)?;
                        self.lock_doors(ctx.scene, true);
                        self.enable_nodes(
                            &mut ctx.scene.graph,
                            &self.nodes_to_enable_on_activation,
                        )?;
                    }
                }
            }
            State::Active => {
                if self.is_all_enemies_dead(ctx.scene)? {
                    self.lock_doors(ctx.scene, false);
                    self.enable_nodes(&mut ctx.scene.graph, &self.nodes_to_enable_on_deactivation)?;
                    self.state = State::Finished;
                }
            }
            State::Finished => {
                // Do nothing.
            }
        }
        Ok(())
    }
}
