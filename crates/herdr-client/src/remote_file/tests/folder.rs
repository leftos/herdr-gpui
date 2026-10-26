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
fn posix_probe_checks_missing_before_not_a_folder_and_prints_five_lines() {
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
    let resolve = script
        .find(&format!(
            "path=$(CDPATH= cd -P -- \"$path\" && pwd -P) || exit {NOT_A_FILE_EXIT}"
        ))
        .unwrap();
    let root = script
        .find(&format!(
            "if [ \"$path\" = / ]; then exit {NOT_A_FILE_EXIT}; fi"
        ))
        .unwrap();
    let top = script.find("top=${path##*/}").unwrap();
    assert!(missing < folder && folder < resolve && resolve < root && root < top);
    assert!(!script.contains("while [ \"${path%/}\""));
    let size = script.find("printf '%s\\n' \"$size\"").unwrap();
    let digest = script.find("printf '%s\\n' \"${sum%% *}\"").unwrap();
    let absent = script.find("printf '%s\\n' '-'").unwrap();
    let printed_top = script.find("printf '%s\\n' \"$top\"").unwrap();
    let zero = script.rfind("printf '%s\\n' 0").unwrap();
    let counted = script
        .find("files=$(find \"$path\" -type f -exec printf '%.0sx' {} + | wc -c | tr -d ' ') || exit 1")
        .unwrap();
    let files = script.find("printf '%s\\n' \"$files\"").unwrap();
    assert!(folder < size && size < digest && size < absent);
    assert!(digest < printed_top && absent < printed_top && printed_top < zero);
    assert!(zero < counted && counted < files);
    assert!(script.contains("case $files in ''|*[!0-9]*) exit 1 ;; esac"));
    // The size sums the regular files' bytes in each stat form, never `du`'s
    // disk blocks.
    assert!(!script.contains("du "));
    assert!(script.contains("find \"$path\" -type f -exec stat -c '%s' -- {} +"));
    assert!(script.contains("find \"$path\" -type f -exec stat -f '%z' -- {} +"));
    assert!(script.contains("find \"$path\" -type f -exec ls -lnd -- {} + | awk '{print $5}'"));
    // GNU stat's `%y` carries nanoseconds, BSD's `%m` whole seconds, and
    // `ls -lnd` is the fallback for a host with neither.
    let gnu = script.find("stat -c '%n %s %y' -- \"$path\"").unwrap();
    let bsd = script.find("stat -f '%N %z %m' -- \"$path\"").unwrap();
    let fallback = script.find("find \"$path\" -exec ls -lnd -- {} +").unwrap();
    assert!(gnu < bsd && bsd < fallback);
    // The probe picks a hasher without exiting when there is none; the push
    // keeps its own refusal.
    assert!(script.contains("hasher=\nif command -v sha256sum"));
    assert!(!script.contains(&format!("exit {NO_HASHER_EXIT}")));
    for hasher in ["sha256sum", "shasum -a 256", "openssl dgst -sha256 -r"] {
        assert!(script.contains(hasher), "{hasher}");
    }
}

#[test]
fn posix_archive_resolves_the_path_before_refusing_the_root() {
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
    let resolve = script
        .find(&format!(
            "path=$(CDPATH= cd -P -- \"$path\" && pwd -P) || exit {NOT_A_FILE_EXIT}"
        ))
        .unwrap();
    assert!(missing < folder && folder < resolve && resolve < root);
    let split = script.find("dir=${path%/*}; base=${path##*/}").unwrap();
    let stream = script
        .find("exec tar -czf - -C \"$dir\" -- \"$base\"")
        .unwrap();
    assert!(root < split && split < stream);
    assert!(!script.contains("while [ \"${path%/}\""));
}

#[test]
fn posix_archive_tells_macos_tar_to_leave_out_appledouble_entries() {
    let script = scripts::posix_archive_script("/tree");
    let disable = script
        .find("\nCOPYFILE_DISABLE=1; export COPYFILE_DISABLE\n")
        .expect("the script sets and exports COPYFILE_DISABLE=1");
    let stream = script
        .find("exec tar -czf - -C \"$dir\" -- \"$base\"")
        .unwrap();
    assert!(disable < stream, "COPYFILE_DISABLE is set before tar runs");
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
fn powershell_probe_checks_missing_before_not_a_folder_and_prints_five_lines() {
    let script = scripts::powershell_probe_script("C:\\tree");
    let directory = script.find("[System.IO.Directory]::Exists($path)").unwrap();
    let file = script.find("[System.IO.File]::Exists($path)").unwrap();
    let not_a_file = script.find(&format!("exit {NOT_A_FILE_EXIT}")).unwrap();
    let missing = script.find(&format!("exit {MISSING_EXIT}")).unwrap();
    assert!(directory < file && file < not_a_file && not_a_file < missing);
    assert!(!script.contains("Write-Output"));
    let utf8 = script
        .find("(New-Object System.Text.UTF8Encoding $false).GetBytes($answer)")
        .unwrap();
    let stdout = script.find("[Console]::OpenStandardOutput()").unwrap();
    assert!(utf8 < stdout);
    assert!(script.contains("$base = [System.IO.Path]::GetFileName($root)"));
    assert!(script.contains(&format!("if ($base -eq '') {{ exit {NOT_A_FILE_EXIT} }}")));
    assert!(script.contains("[int]$skipped = 0"));
    assert!(script.contains("$skipped++; continue"));
    assert!(script.contains("[long]$files = 0"));
    let counted = script.find("$files++").unwrap();
    let summed = script.find("$total += $info.Length").unwrap();
    assert!(summed < counted, "only the regular-file branch counts");
    assert!(script.contains("([long]$skipped).ToString(), $files.ToString()))"));
    assert!(script.contains("FileLength = -1"));
    assert!(script.contains("Ticks = 0"));
    assert!(script.contains("FileAttributes]::ReparsePoint"));
    assert!(script.contains("Sort-Object Relative"));
    assert!(script.contains("SHA256]::Create()"));
    assert!(
        !script.contains("Math]::Ceiling"),
        "the size is bytes, not KiB"
    );
    assert!(script.contains("@($total.ToString(), $digest,"));
    assert!(script.contains("ToLowerInvariant"));
}

#[test]
fn powershell_archive_builds_a_zip_beside_its_output_bound() {
    let script = scripts::powershell_archive_script("C:\\tree", 42);
    assert!(script.contains("ZipFile]::Open($zip, 'Create')"));
    assert!(script.contains("$base = [System.IO.Path]::GetFileName($root)"));
    assert!(script.contains(&format!("if ($base -eq '') {{ exit {NOT_A_FILE_EXIT} }}")));
    assert!(
        script.contains("[void]$archive.CreateEntry($base + '/' + $relative + '/')"),
        "an empty folder is written as a directory entry"
    );
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
