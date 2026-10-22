//! `[remote_files]` size limit for copying a folder printed in an SSH pane.
use serde::Deserialize;

/// How large a remote folder's archive may get before the app refuses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteFilesConfig {
    /// Refuse an archive larger than this many megabytes.
    pub folder_transfer_cap_mb: u32,
}

impl Default for RemoteFilesConfig {
    fn default() -> Self {
        Self {
            folder_transfer_cap_mb: 512,
        }
    }
}

/// Only explicitly configured keys override the defaults.
#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct RemoteFilesSettings {
    #[serde(deserialize_with = "optional_transfer_cap_mb")]
    folder_transfer_cap_mb: Option<u32>,
}

impl RemoteFilesSettings {
    pub(super) fn resolve(self, base: RemoteFilesConfig) -> RemoteFilesConfig {
        RemoteFilesConfig {
            folder_transfer_cap_mb: self
                .folder_transfer_cap_mb
                .unwrap_or(base.folder_transfer_cap_mb),
        }
    }
}

fn optional_transfer_cap_mb<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<u32>, D::Error> {
    transfer_cap_mb(d).map(Some)
}

fn transfer_cap_mb<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let value = u32::deserialize(d)?;
    if !(1..=8192).contains(&value) {
        return Err(serde::de::Error::custom(
            "remote_files.folder_transfer_cap_mb must be between 1 and 8192",
        ));
    }
    Ok(value)
}
