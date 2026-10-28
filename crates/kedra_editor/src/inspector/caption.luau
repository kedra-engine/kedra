-- Host-authorized composition of the shared native property controls.
-- Rust owns parsing, validation, source acceptance, history and preview refresh.
function run(ctx, args)
    ctx.fields.add("text", "Text")
    ctx.fields.add("justify", "Alignment")
    ctx.fields.add("wrap", "Wrap")
end
