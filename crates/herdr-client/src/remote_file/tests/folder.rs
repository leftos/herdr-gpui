//! The generated folder scripts: quoting, home expansion, the two-line probe
//! answer, and the archive's shape.

use super::super::folder as scripts;
use super::*;
use crate::script::shell_quote;

#[test]
fn posix_folder_scripts_quote_the_path_as_one_word() {
    let probe = scripts::posix_probe_script(HOSTILE_NAME);
    assert!(probe.starts_with(&format!("path={}\n", shell_quote(HOSTILE_NAME))));
    let archive = scripts::posix_archive_script(HOSTILE_NAME);
    assert!(archive.starts_with(&format!("path={}\n", shell_quote(HOSTILE_NAME))));
    assert!(scripts::posix_probe_script("/srv/it's").starts_with("path='/srv/it'\\''s'\n"));
}

#[test]
fn posix_folder_scripts_expand_only_a_leading_home() {
    let probe = |path| scripts::posix_probe_script(path);
    for path in ["~/a b", "~/-x"] {
        assert!(probe(path).starts_with("path=\"$HOME\"/'"), "{path}");
    }
    assert!(probe("~").starts_with("path=\"$HOME\"\n"));
    assert!(probe("~root/x").starts_with("path='~root/x'\n"));
    assert!(probe("/srv/~/x").starts_with("path='/srv/~/x'\n"));
    for path in ["~/a b", "~"] {
        assert_eq!(
            scripts::posix_archive_script(path).lines().next(),
            probe(path).lines().next()
        );
    }
}

#[test]
fn posix_probe_checks_missing_before_not_a_folder_and_prints_two_lines() {
    let script = scripts::posix_probe_script("/tree");
    let missing = script
        .find(&format!(
            "if [ ! -e \"$path\" ]; then exit {MISSING_EXIT}; fi"
        ))
        .unwrap();
    let folder = script
        .find(&format!(
            "if [ ! -d \"$path\" ]; then exit {NOT_A_FILE_EXIT}; fi"
        ))
        .unwrap();
    assert!(missing < folder);
    let size = script.find("printf '%s\\n' \"$size\"").unwrap();
    let digest = script.find("printf '%s\\n' \"${sum%% *}\"").unwrap();
    let absent = script.find("printf '%s\\n' '-'").unwrap();
    assert!(folder < size && size < digest && size < absent);
    assert!(script.contains("du -sk -- \"$path\""));
    assert!(script.contains("find \"$path\" -exec ls -lnd -- {} + | LC_ALL=C sort | $hasher"));
    // The probe picks a hasher without exiting when there is none; the push
    // keeps its own refusal.
    assert!(script.contains("hasher=\nif command -v sha256sum"));
    assert!(!script.contains(&format!("exit {NO_HASHER_EXIT}")));
    for hasher in ["sha256sum", "shasum -a 256", "openssl dgst -sha256 -r"] {
        assert!(script.contains(hasher), "{hasher}");
    }
}

#[test]
fn posix_archive_refuses_the_root_and_strips_trailing_slashes() {
    let script = scripts::posix_archive_script("/tree/");
    let missing = script
        .find(&format!(
            "if [ ! -e \"$path\" ]; then exit {MISSING_EXIT}; fi"
        ))
        .unwrap();
    let folder = script
        .find(&format!(
            "if [ ! -d \"$path\" ]; then exit {NOT_A_FILE_EXIT}; fi"
        ))
        .unwrap();
    let root = script
        .find(&format!(
            "if [ \"$path\" = / ]; then exit {NOT_A_FILE_EXIT}; fi"
        ))
        .unwrap();
    assert!(missing < folder && folder < root);
    let strip = script
        .find("while [ \"${path%/}\" != \"$path\" ]; do path=${path%/}; done")
        .unwrap();
    let split = script.find("dir=${path%/*}; base=${path##*/}").unwrap();
    let stream = script
        .find("exec tar -czf - -C \"$dir\" -- \"$base\"")
        .unwrap();
    assert!(root < strip && strip < split && split < stream);
}

#[test]
fn powershell_folder_scripts_quote_the_path_as_a_literal() {
    let probe = scripts::powershell_probe_script(HOSTILE_NAME);
    assert!(probe.contains(&format!(
        "\n$path = {}\n",
        powershell::literal(HOSTILE_NAME)
    )));
    let archive = scripts::powershell_archive_script(HOSTILE_NAME, 10);
    assert!(archive.contains(&format!(
        "\n$path = {}\n",
        powershell::literal(HOSTILE_NAME)
    )));
    let combine = "[System.IO.Path]::Combine($env:USERPROFILE, 'a b')";
    assert!(scripts::powershell_probe_script("~\\a b").contains(combine));
    assert!(scripts::powershell_probe_script("~/a b").contains(combine));
    assert!(scripts::powershell_probe_script("~").contains("\n$path = $env:USERPROFILE\n"));
    assert!(scripts::powershell_probe_script("~x\\a").contains("\n$path = '~x\\a'\n"));
}

#[test]
fn powershell_probe_checks_missing_before_not_a_folder_and_prints_two_lines() {
    let script = scripts::powershell_probe_script("C:\\tree");
    let directory = script.find("[System.IO.Directory]::Exists($path)").unwrap();
    let file = script.find("[System.IO.File]::Exists($path)").unwrap();
    let not_a_file = script.find(&format!("exit {NOT_A_FILE_EXIT}")).unwrap();
    let missing = script.find(&format!("exit {MISSING_EXIT}")).unwrap();
    assert!(directory < file && file < not_a_file && not_a_file < missing);
    assert_eq!(script.matches("Write-Output").count(), 2);
    assert!(script.contains("FileAttributes]::ReparsePoint"));
    assert!(script.contains("Sort-Object Relative"));
    assert!(script.contains("SHA256]::Create()"));
    assert!(script.contains("Math]::Ceiling"));
    assert!(script.contains("ToLowerInvariant"));
}

#[test]
fn powershell_archive_builds_a_zip_beside_its_output_bound() {
    let script = scripts::powershell_archive_script("C:\\tree", 42);
    assert!(script.contains("ZipFile]::Open($zip, 'Create')"));
    assert!(script.contains(
        "ZipFileExtensions]::CreateEntryFromFile($archive, $item, $base + '/' + $relative)"
    ));
    assert!(script.contains("$file.Length -gt 42) { exit 68 }"));
    assert!(script.contains("[Console]::OpenStandardOutput()"));
    assert!(script.contains("Remove-Item -LiteralPath $zip -Force"));
    assert!(
        scripts::powershell_archive_script("C:\\tree", u64::MAX)
            .contains(&format!("-gt {})", i64::MAX))
    );
}
