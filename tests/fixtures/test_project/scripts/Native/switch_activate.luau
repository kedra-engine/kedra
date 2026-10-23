--!nolint FunctionUnused
function run(ctx: KedraUiContext, args: {color: {number}})
    ctx.ui.set_background(ctx.entity, args.color)
    ctx.log.info("Switch activated")
end
