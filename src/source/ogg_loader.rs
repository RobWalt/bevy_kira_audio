use anyhow::Result;
use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, AsyncReadExt, LoadContext};
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use kira::sound::FromFileError;
use std::io::Cursor;
use thiserror::Error;

use crate::source::AudioSource;

/// Possible errors that can be produced by [`OggLoader`]
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum OggLoaderError {
    /// An [IO Error](std::io::Error)
    #[error("Could not read the file: {0}")]
    Io(#[from] std::io::Error),
    /// An Error loading sound from a file. See [`FromFileError`]
    #[error("Error while loading a sound: {0}")]
    FileError(#[from] FromFileError),
}

#[derive(Default)]
pub struct OggLoader;

impl AssetLoader for OggLoader {
    type Asset = AudioSource;
    type Settings = ();
    type Error = OggLoaderError;

    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _settings: &'a (),
        _load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut sound_bytes = vec![];
        reader.read_to_end(&mut sound_bytes).await?;
        let sound =
            StaticSoundData::from_cursor(Cursor::new(sound_bytes), StaticSoundSettings::default())?;
        Ok(AudioSource { sound })
    }

    fn extensions(&self) -> &[&str] {
        &["ogg", "oga", "spx"]
    }
}
