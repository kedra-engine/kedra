//! Blocking image codecs and persistent cache I/O stay on Bevy's worker pool.
use super::*;
use kedra_wakeup::{Task, Wakeup};
use std::io::Cursor;

pub(super) struct Disk {
    pub request: Request,
    pub reading: bool,
    pub task: Task<Result<Option<Image>, String>>,
}
fn key(request: &Request) -> kedra_cache::Key {
    kedra_cache::Key::new(
        "asset-thumbnail",
        1,
        &[
            request.path.as_bytes(),
            request.revision.as_bytes(),
            &SIZE.to_le_bytes(),
            &[request.kind as u8],
        ],
    )
}
impl Disk {
    pub fn read(world: &World, request: Request, store: kedra_cache::Store) -> Self {
        let key = key(&request);
        let task = Wakeup::for_world(world).spawn(bevy::tasks::IoTaskPool::get(), async move {
            let Some(bytes) = store.get(&key)? else {
                return Ok(None);
            };
            let mut reader =
                image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(SIZE);
            limits.max_image_height = Some(SIZE);
            limits.max_alloc = Some(u64::from(SIZE) * u64::from(SIZE) * 8);
            reader.limits(limits);
            // Codec failures are derived-cache misses, not failures of the source.
            Ok(reader
                .decode()
                .ok()
                .map(|image| Image::from_dynamic(image, true, default())))
        });
        Self {
            request,
            reading: true,
            task,
        }
    }
    pub fn write(world: &World, request: Request, store: kedra_cache::Store, image: Image) -> Self {
        let key = key(&request);
        let task = Wakeup::for_world(world).spawn(bevy::tasks::IoTaskPool::get(), async move {
            let mut bytes = Cursor::new(Vec::new());
            image
                .try_into_dynamic()
                .map_err(|e| e.to_string())?
                .write_to(&mut bytes, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            store.put(&key, bytes.get_ref())?;
            Ok(None)
        });
        Self {
            request,
            reading: false,
            task,
        }
    }
}
