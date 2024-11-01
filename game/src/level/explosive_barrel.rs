use crate::level::hit_box::HitBoxMessage;
use fyrox::generic_animation::machine::Event;
use fyrox::{
    core::variable::InheritableVariable,
    core::{
        pool::Handle, reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*,
        ImmutableString,
    },
    generic_animation::machine::Parameter,
    graph::SceneGraph,
    resource::model::{ModelResource, ModelResourceExtension},
    scene::{animation::absm::AnimationBlendingStateMachine, node::Node},
    script::{ScriptContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait},
};

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "1bd90488-7a17-430e-9b35-dc0a9a1a2f58")]
#[visit(optional)]
pub struct ExplosiveBarrel {
    health: InheritableVariable<f32>,
    normal_state: InheritableVariable<ImmutableString>,
    burning_state: InheritableVariable<ImmutableString>,
    exploded_state: InheritableVariable<ImmutableString>,
    state_machine: InheritableVariable<Handle<Node>>,
    explosion_prefab: InheritableVariable<Option<ModelResource>>,
}

impl Default for ExplosiveBarrel {
    fn default() -> Self {
        Self {
            health: 100.0.into(),
            normal_state: ImmutableString::new("Normal").into(),
            burning_state: ImmutableString::new("Burning").into(),
            exploded_state: ImmutableString::new("Exploded").into(),
            state_machine: Default::default(),
            explosion_prefab: Default::default(),
        }
    }
}

impl ScriptTrait for ExplosiveBarrel {
    fn on_start(&mut self, ctx: &mut ScriptContext) {
        ctx.message_dispatcher
            .subscribe_to::<HitBoxMessage>(ctx.handle);
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        _ctx: &mut ScriptMessageContext,
    ) {
        let Some(hit_box_message) = message.downcast_ref::<HitBoxMessage>() else {
            return;
        };

        *self.health -= hit_box_message.amount;
    }

    fn on_update(&mut self, context: &mut ScriptContext) {
        let position = context.scene.graph[context.handle].global_position();

        if let Some(absm) = context
            .scene
            .graph
            .try_get_mut_of_type::<AnimationBlendingStateMachine>(*self.state_machine)
        {
            let machine = absm.machine_mut();
            machine.set_parameter("IsDamaged", Parameter::Rule(*self.health <= 0.0));

            if let Some(layer) = machine.layers_mut().first_mut() {
                while let Some(event) = layer.pop_event() {
                    if let Event::StateEnter(state) = event {
                        if let Some(state_ref) = layer.states().try_borrow(state) {
                            if state_ref.name.as_str() == self.exploded_state.as_str() {
                                if let Some(explosion_prefab) = self.explosion_prefab.as_ref() {
                                    explosion_prefab.instantiate_at(
                                        context.scene,
                                        position,
                                        Default::default(),
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
