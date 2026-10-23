--!nolint FunctionUnused
type Context = KedraBehaviorContext & {params: {color: {number}}}
type State = {activation: KedraSequenceHandle?}

function start(self: State, ctx: Context)
    ctx.events.on(ctx.entity, "activate", function(_payload)
        if self.activation then
            ctx.sequences.cancel(self.activation)
        end
        self.activation = ctx.sequences.start("./switch_activate", {color = ctx.params.color})
    end)
end
