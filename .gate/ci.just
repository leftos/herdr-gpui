# Fork-only (leftos/patches): `ci` minus the four Credential Manager tests,
# which need a console logon and run as their own desktop command:
#   cargo test --locked -p herdr-gpui --all-features credential_manager
ci-routed: format-check check-file-size check-sorted lint test-routed test-mockup-wait

test-routed:
    cargo test --locked --workspace -- --skip credential_manager
    cargo test --locked --workspace --all-features -- --skip credential_manager
