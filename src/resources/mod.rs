use std::{
    fs,
    path::{Path, PathBuf},
};

pub mod buffer;
pub mod camera;
pub mod font;
pub mod model;
pub mod texture;
pub mod vertex;
pub mod light;


pub trait Resources {
    fn load_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String>;
    fn load_binary(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>>;
}

pub struct FsResources {
    base_dir: PathBuf,
}

impl FsResources {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_owned(),
        }
    }
}

impl Resources for FsResources {
    fn load_binary(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
        // TODO: WASM
        let full_path = self.base_dir.join(path);
        log::info!("load_binary({})", full_path.display());
        Ok(fs::read(full_path)?)
    }

    fn load_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        // TODO: WASM
        let full_path = self.base_dir.join(path);
        log::info!("load_string({})", full_path.display());
        Ok(fs::read_to_string(full_path)?)
    }
}
