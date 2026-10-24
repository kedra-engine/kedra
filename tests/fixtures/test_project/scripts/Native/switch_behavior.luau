--!nolint FunctionUnused
-- The native switch host authorizes this Behavior only through attach.
type Context = BehaviorContext & {entity: EntityHandle, params: {color: {number}}}
type State = {activation: SequenceHandle?}

function start(self: State, ctx: Context)
    ctx.events.on(ctx.entity, "activate", function(_payload)
        if self.activation then
            ctx.sequences.cancel(self.activation)
        end
        self.activation = ctx.sequences.start("./switch_activate", {color = ctx.params.color})
    end)
end
