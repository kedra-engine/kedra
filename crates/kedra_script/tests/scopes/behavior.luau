--!strict
--!nolint FunctionUnused
type State = {attached: boolean?}

function start(self: State, ctx: BehaviorContext)
    self.attached = ctx.entity ~= nil
    ctx.log.info("behavior attachment", {attached = self.attached})
    if ctx.entity then
        ctx.events.on(ctx.entity, "activate", function()
            ctx.log.info("attached activation")
        end)
    end
end

function update(self: State, ctx: BehaviorContext, _dt: number)
    assert(self.attached == (ctx.entity ~= nil), "Attachment changed during the run")
end
