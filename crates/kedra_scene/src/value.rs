//! Preserve typed RON source values with the maintained RON parser; no custom lexer.
pub(crate) fn parse(source: &str) -> crate::Result<Box<ron::value::RawValue>> {
    if source.len() > 65_536 {
        return Err(crate::failure("component value exceeds 64 KiB"));
    }
    Ok(ron::value::RawValue::from_ron(source)?.trim().to_owned())
}
