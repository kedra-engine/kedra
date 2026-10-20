//! The fixture uses the same maintained lightweight Windows resource mechanism.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=KEDRA_WINDOWS_ICON");
    println!("cargo:rerun-if-changed=icon.ico");
    #[cfg(target_os = "windows")]
    {
        let icon = std::env::var_os("KEDRA_WINDOWS_ICON")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("icon.ico"));
        let icon = icon.to_str().ok_or("Native icon path must be Unicode")?;
        println!("cargo:rerun-if-changed={icon}");
        winresource::WindowsResource::new()
            .set_icon(icon)
            .set("ProductName", "Independent Distribution Fixture")
            .set("FileDescription", "Independent distribution acceptance fixture")
            .compile()?;
    }
    Ok(())
}
