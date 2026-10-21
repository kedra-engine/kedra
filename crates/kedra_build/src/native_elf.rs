//! Maintained ELF parsing owns section/dynamic-table decoding.
use object::{Object, ObjectSection, ObjectSymbol};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) fn identity(bytes: &[u8]) -> Result<String, String> {
    let elf = object::read::elf::ElfFile64::<object::Endianness>::parse(bytes)
        .map_err(|e| format!("Native ELF identity: {e}"))?;
    let mut sections = BTreeMap::new();
    for section in elf.sections() {
        let object::SectionFlags::Elf { sh_flags, sh_type } = section.flags() else {
            return Err("Native section is not ELF".into());
        };
        let name = section.name_bytes().map_err(|e| e.to_string())?;
        // linuxdeploy/patchelf move sections and update RPATH/dynamic strings.
        // Preserve identity of all loaded code/data/notes, not physical offsets.
        if sh_flags.0 & object::elf::SHF_ALLOC.0 == 0
            || name == b".dynamic"
            || name == b".dynstr"
            || name == b".dynsym"
        {
            continue;
        }
        let content: [u8; 32] = Sha256::digest(section.data().map_err(|e| e.to_string())?).into();
        if sections
            .insert(name, (sh_flags.0, sh_type.0, section.size(), content))
            .is_some()
        {
            return Err("Duplicate loaded ELF section name".into());
        }
    }
    if sections.is_empty() {
        return Err("Native ELF has no loaded sections".into());
    }
    let mut identity = Sha256::new();
    identity.update(
        format!(
            "ELF-identity-v1:{:?}:{:?}:{:?}:{}:{}",
            elf.architecture(),
            elf.kind(),
            elf.flags(),
            elf.entry(),
            elf.is_little_endian()
        )
        .as_bytes(),
    );
    for (name, (flags, kind, size, content)) in sections {
        identity.update((name.len() as u64).to_le_bytes());
        identity.update(name);
        identity.update(flags.to_le_bytes());
        identity.update(kind.to_le_bytes());
        identity.update(size.to_le_bytes());
        identity.update(content);
    }
    // patchelf can renumber sections. Bind actual symbol names/definitions rather
    // than raw section indices or string-table offsets in .dynsym.
    for symbol in elf.dynamic_symbols() {
        let name = symbol.name_bytes().map_err(|e| e.to_string())?;
        identity.update((name.len() as u64).to_le_bytes());
        identity.update(name);
        identity.update(symbol.address().to_le_bytes());
        identity.update(symbol.size().to_le_bytes());
        identity.update(
            format!(
                "{:?}:{:?}:{:?}:{}",
                symbol.kind(),
                symbol.scope(),
                symbol.flags(),
                symbol.is_weak()
            )
            .as_bytes(),
        );
        match symbol.section() {
            object::SymbolSection::Section(index) => identity.update(
                elf.section_by_index(index)
                    .map_err(|e| e.to_string())?
                    .name_bytes()
                    .map_err(|e| e.to_string())?,
            ),
            other => identity.update(format!("{other:?}").as_bytes()),
        }
    }
    // Needed libraries and SONAME must not hide behind the mutable string table.
    let dynamic = elf.elf_dynamic_table().map_err(|e| e.to_string())?;
    for entry in dynamic
        .iter()
        .filter(|entry| entry.tag == object::elf::DT_NEEDED || entry.tag == object::elf::DT_SONAME)
    {
        let name = dynamic.string(entry).map_err(|e| e.to_string())?;
        identity.update(entry.tag.0.to_le_bytes());
        identity.update((name.len() as u64).to_le_bytes());
        identity.update(name);
    }
    Ok(format!("{:x}", identity.finalize()))
}

#[cfg(test)]
pub(crate) fn fixture(code: u8) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut elf = object::write::Object::new(
        object::BinaryFormat::Elf,
        object::Architecture::X86_64,
        object::Endianness::Little,
    );
    let text = elf.add_section(Vec::new(), b".text".to_vec(), object::SectionKind::Text);
    elf.append_section_data(text, &[code; 32], 16);
    Ok(elf.write()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires real distro and linuxdeploy-packaged library fixture paths"]
    fn retained_library_relocation_preserves_identity_but_replacement_code_does_not()
    -> Result<(), Box<dyn std::error::Error>> {
        let original = std::fs::read(
            std::env::var_os("KEDRA_TEST_LIBRARY_ORIGINAL")
                .ok_or("Set KEDRA_TEST_LIBRARY_ORIGINAL")?,
        )?;
        let bundled = std::fs::read(
            std::env::var_os("KEDRA_TEST_LIBRARY_BUNDLED")
                .ok_or("Set KEDRA_TEST_LIBRARY_BUNDLED")?,
        )?;
        assert_ne!(
            original, bundled,
            "Real fixture must exercise packager relocation, not identical inputs"
        );
        assert_eq!(identity(&original)?, identity(&bundled)?);
        let file = object::File::parse(original.as_slice())?;
        let (offset, _) = file
            .section_by_name(".text")
            .ok_or("Text section missing")?
            .file_range()
            .ok_or("Text data missing")?;
        let mut changed = original.clone();
        changed[offset as usize] ^= 1;
        assert_ne!(identity(&changed)?, identity(&bundled)?);
        Ok(())
    }
}
