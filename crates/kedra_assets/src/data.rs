//! Opaque delivered data, including legal notices shipped with fonts.
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
/// Byte data without domain parsing or editor dependencies.
#[derive(Asset, TypePath)]
pub struct DataAsset {
    /// Delivered, verified bytes.
    pub bytes: Vec<u8>,
}
/// Native loader for data whose interpretation belongs to the consuming plugin.
#[derive(Default, TypePath)]
pub struct DataLoader;
impl AssetLoader for DataLoader {
    type Asset = DataAsset;
    type Settings = ();
    type Error = std::io::Error;
    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        _: &mut LoadContext<'_>,
    ) -> Result<DataAsset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(DataAsset { bytes })
    }
    fn extensions(&self) -> &[&str] {
        &["txt", "json", "ron"]
    }
}
