use super::*;

#[test]
fn remote_folder_sizes_default_and_overlay_like_notifications() -> anyhow::Result<()> {
    for config in [
        Config::default(),
        Config::parse("")?,
        Config::parse(DEFAULT_CONFIG)?,
    ] {
        assert_eq!(
            config.remote_files,
            RemoteFilesConfig {
                folder_transfer_cap_mb: 512,
            }
        );
    }
    let config = Config::parse("[remote_files]\nfolder_transfer_cap_mb = 200")?;
    assert_eq!(
        config.remote_files,
        RemoteFilesConfig {
            folder_transfer_cap_mb: 200,
        }
    );
    Ok(())
}

#[test]
fn remote_folder_sizes_accept_their_bounds_and_refuse_one_past() -> anyhow::Result<()> {
    for text in [
        "[remote_files]\nfolder_transfer_cap_mb = 1",
        "[remote_files]\nfolder_transfer_cap_mb = 8192",
    ] {
        assert!(Config::parse(text).is_ok(), "rejected {text:?}");
    }
    for text in [
        "[remote_files]\nfolder_transfer_cap_mb = 0",
        "[remote_files]\nfolder_transfer_cap_mb = 8193",
    ] {
        assert!(Config::parse(text).is_err(), "accepted {text:?}");
    }
    Ok(())
}

#[test]
fn remote_folder_size_range_messages_name_their_key() -> anyhow::Result<()> {
    let text = "[remote_files]\nfolder_transfer_cap_mb = 0";
    let error = Config::parse(text)
        .err()
        .ok_or_else(|| anyhow::anyhow!("accepted {text:?}"))?;
    assert!(
        error
            .to_string()
            .contains("remote_files.folder_transfer_cap_mb must be between 1 and 8192"),
        "{error}"
    );
    Ok(())
}

#[test]
fn unknown_remote_files_keys_are_reported() -> anyhow::Result<()> {
    assert_eq!(
        Config::parse("[remote_files]\nfuture = 1")?.unknown_keys,
        ["remote_files.future"]
    );
    Ok(())
}
