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
        Ok(fs::read(self.base_dir.join(path))?)
    }

    fn load_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        // TODO: WASM
        Ok(fs::read_to_string(self.base_dir.join(path))?)
    }
}
