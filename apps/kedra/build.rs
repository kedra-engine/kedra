//! Compile local Windows resources only; application bundling belongs to kedra_build.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(target_os = "windows")]
    windows_resources()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_resources() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=KEDRA_WINDOWS_ICON");
    println!("cargo:rerun-if-changed=resources/kedra.ico");
    let icon = std::env::var_os("KEDRA_WINDOWS_ICON")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("resources/kedra.ico"));
    let icon = icon
        .to_str()
        .ok_or("Windows resource compiler requires a Unicode icon path")?;
    println!("cargo:rerun-if-changed={icon}");
    winresource::WindowsResource::new()
        .set_icon(icon)
        .set("ProductName", "Kedra")
        .set("ProductVersion", env!("CARGO_PKG_VERSION"))
        .set("FileDescription", "Kedra game development environment")
        .set("CompanyName", "Kedra contributors")
        .set("OriginalFilename", "kedra.exe")
        .compile()?;
    Ok(())
}
