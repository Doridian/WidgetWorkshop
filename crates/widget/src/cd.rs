//! The game's CD as the engine reads it: an ISO image (through
//! `platform::data`) or a folder; and `--export`.

use std::io;
use std::path::Path;

use engine::assets::{Assets, Dir, Module, Source, rt};
use platform::fs::Source as _;

/// A CD image as a [`Source`].
struct Image(platform::data::CdImage);

impl Source for Image {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.0.read(path).ok()
    }

    fn list(&self, path: &str) -> Vec<String> {
        self.0
            .list(path)
            .map(|l| l.into_iter().map(|(name, _)| name).collect())
            .unwrap_or_default()
    }
}

/// The CD at `path`: an image file or a folder.
///
/// # Errors
/// If it's neither, or the image isn't ISO 9660.
pub fn open(path: &Path) -> Result<Box<dyn Source>, String> {
    if path.is_dir() {
        return Ok(Box::new(Dir(path.to_owned())));
    }
    if !path.is_file() {
        return Err("no such file or folder".into());
    }
    platform::data::CdImage::open_file(path).map(|cd| Box::new(Image(cd)) as Box<dyn Source>)
}

/// The file extension a resource of type `typ` is exported with.
fn extension(m: Module, typ: u32) -> &'static str {
    match (m, typ) {
        (_, rt::PICTURES) => "hr",
        (_, rt::SOUND) => "wav",
        (_, rt::TEXT) => "txt",
        (_, rt::WIDGET) => "wgt",
        (_, 2) => "bmp",
        _ => "bin",
    }
}

/// Writes every resource of `assets` under `dir` as
/// `<MODULE>/<type>/<id>.<ext>`; returns how many.
///
/// # Errors
/// If a file can't be written.
pub fn export(assets: &Assets, dir: &Path) -> io::Result<usize> {
    let mut n = 0;
    for (m, typ, id, bytes) in assets.all() {
        let d = dir.join(m.dir()).join(typ.to_string());
        std::fs::create_dir_all(&d)?;
        std::fs::write(d.join(format!("{id}.{}", extension(m, typ))), bytes)?;
        n += 1;
    }
    Ok(n)
}
