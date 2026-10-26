//! The folder probe and archive scripts. A POSIX host runs them under
//! `/bin/sh`; a Windows OpenSSH host runs them under Windows PowerShell. The
//! probe prints five lines: a folder's size, a fingerprint of its tree, the
//! name its archive's entries start with, the entries its archive leaves out
//! as links, and the regular files in its tree; the archive streams a gzipped
//! tar (POSIX) or a zip (PowerShell) of the folder to stdout. Both shells use
//! the same exit-code contract as the file transfer scripts.

use super::{
    MISSING_EXIT, NOT_A_FILE_EXIT, TOO_LARGE_EXIT,
    posix::{choose_hasher, path_assignment},
    powershell::{PREAMBLE, path_expression},
};

/// The `/bin/sh` script that prints the folder's size, the sum of its regular
/// files' bytes, a SHA-256 of a sorted metadata listing of its tree (or `-`
/// when the host has no SHA-256 tool), the name its archive's entries start
/// with, `0` for the links its archive leaves out, and the number of regular
/// files in the tree. GNU `stat -c`, BSD `stat -f`, or `ls -lnd` on a host
/// with neither, gives both the sizes and the listing; `du` is not used, as
/// it counts disk blocks and GNU and BSD spell its byte option differently.
/// `find -type f` neither follows nor counts links, and `%.0sx` prints one
/// `x` per path, so a file name holding a newline still counts once.
pub(super) fn posix_probe_script(path: &str) -> String {
    format!(
        r#"{assign}
if [ ! -e "$path" ]; then exit {MISSING_EXIT}; fi
if [ ! -d "$path" ]; then exit {NOT_A_FILE_EXIT}; fi
path=$(CDPATH= cd -P -- "$path" && pwd -P) || exit {NOT_A_FILE_EXIT}
if [ "$path" = / ]; then exit {NOT_A_FILE_EXIT}; fi
top=${{path##*/}}
if stat -c '%n %s %y' -- "$path" >/dev/null 2>&1; then form=gnu
elif stat -f '%N %z %m' -- "$path" >/dev/null 2>&1; then form=bsd
else form=ls
fi
case $form in
gnu) sizes=$(find "$path" -type f -exec stat -c '%s' -- {{}} +) ;;
bsd) sizes=$(find "$path" -type f -exec stat -f '%z' -- {{}} +) ;;
*) sizes=$(find "$path" -type f -exec ls -lnd -- {{}} + | awk '{{print $5}}') ;;
esac || exit 1
size=$(printf '%s\n' "$sizes" | awk '{{total += $1}} END {{printf "%.0f\n", total}}') || exit 1
case $size in ''|*[!0-9]*) exit 1 ;; esac
printf '%s\n' "$size"
{hasher}
if [ -n "$hasher" ]; then
case $form in
gnu) listing=$(find "$path" -exec stat -c '%n %s %y' -- {{}} +) ;;
bsd) listing=$(find "$path" -exec stat -f '%N %z %m' -- {{}} +) ;;
*) listing=$(find "$path" -exec ls -lnd -- {{}} +) ;;
esac || exit 1
sum=$(printf '%s\n' "$listing" | LC_ALL=C sort | $hasher) || exit 1
printf '%s\n' "${{sum%% *}}"
else
printf '%s\n' '-'
fi
printf '%s\n' "$top"
printf '%s\n' 0
files=$(find "$path" -type f -exec printf '%.0sx' {{}} + | wc -c | tr -d ' ') || exit 1
case $files in ''|*[!0-9]*) exit 1 ;; esac
printf '%s\n' "$files""#,
        assign = path_assignment(path),
        hasher = choose_hasher(None),
    )
}

/// The `/bin/sh` script that streams the folder as a gzipped tar whose entries
/// start with the folder's name. Both POSIX scripts resolve a link to the
/// folder to its physical path first, so the archive holds the target's tree
/// rather than the link, while links inside the tree stay links. `/`, or a
/// link to it, is refused: extracting it would replace the host's whole tree.
/// `COPYFILE_DISABLE=1` stops macOS `tar` adding an AppleDouble `._` entry
/// for every file carrying extended attributes; other `tar`s ignore it.
pub(super) fn posix_archive_script(path: &str) -> String {
    format!(
        r#"{assign}
if [ ! -e "$path" ]; then exit {MISSING_EXIT}; fi
if [ ! -d "$path" ]; then exit {NOT_A_FILE_EXIT}; fi
path=$(CDPATH= cd -P -- "$path" && pwd -P) || exit {NOT_A_FILE_EXIT}
if [ "$path" = / ]; then exit {NOT_A_FILE_EXIT}; fi
dir=${{path%/*}}; base=${{path##*/}}
if [ -z "$dir" ]; then dir=/; fi
COPYFILE_DISABLE=1; export COPYFILE_DISABLE
exec tar -czf - -C "$dir" -- "$base""#,
        assign = path_assignment(path),
    )
}

/// The PowerShell script that prints the folder's size, the sum of its regular
/// files' bytes, a SHA-256 of
/// its sorted metadata listing, the name its archive's entries start with, the
/// number of reparse points it skipped, and the number of regular files it
/// summed; directories and reparse points are not counted. Directories are
/// walked with an explicit stack because `Get-ChildItem -Recurse` follows
/// junctions on
/// Windows PowerShell 5.1; reparse points are skipped here. The answer is
/// written to stdout as UTF-8 bytes, since `Write-Output` encodes with the
/// console's code page and would garble a non-ASCII folder name.
pub(super) fn powershell_probe_script(path: &str) -> String {
    format!(
        r#"{PREAMBLE}
$path = {path}
if (-not [System.IO.Directory]::Exists($path)) {{
    if ([System.IO.File]::Exists($path)) {{ exit {NOT_A_FILE_EXIT} }}
    exit {MISSING_EXIT}
}}
$root = [System.IO.Path]::GetFullPath($path).TrimEnd([char[]]@('\', '/'))
if ($root -eq '') {{ $root = '/' }}
$base = [System.IO.Path]::GetFileName($root)
if ($base -eq '') {{ exit {NOT_A_FILE_EXIT} }}
$prefix = $root
if ($root -ne '/') {{ $prefix = $root + [System.IO.Path]::DirectorySeparatorChar }}
$directories = New-Object System.Collections.Stack
$directories.Push($path)
$entries = New-Object System.Collections.Generic.List[object]
[long]$total = 0
[int]$skipped = 0
[long]$files = 0
while ($directories.Count -gt 0) {{
    $directory = $directories.Pop()
    $normalized = [System.IO.Path]::GetFullPath($directory).TrimEnd([char[]]@('\', '/'))
    if ($normalized -ne $root) {{
        $relativeDirectory = $normalized.Substring($prefix.Length).Replace('\', '/')
        $entries.Add([pscustomobject]@{{
            Relative = $relativeDirectory
            FileLength = -1
            Ticks = 0
        }})
    }}
    foreach ($item in [System.IO.Directory]::GetFileSystemEntries($directory)) {{
        $attributes = [System.IO.File]::GetAttributes($item)
        if (($attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {{ $skipped++; continue }}
        if (($attributes -band [System.IO.FileAttributes]::Directory) -ne 0) {{
            $directories.Push($item)
        }} else {{
            $info = [System.IO.FileInfo]::new($item)
            $total += $info.Length
            $files++
            $relative = [System.IO.Path]::GetFullPath($item).Substring($prefix.Length).Replace('\', '/')
            $entries.Add([pscustomobject]@{{
                Relative = $relative
                FileLength = $info.Length
                Ticks = $info.LastWriteTimeUtc.Ticks
            }})
        }}
    }}
}}
$sha = [System.Security.Cryptography.SHA256]::Create()
try {{
    foreach ($entry in ($entries | Sort-Object Relative)) {{
        $line = $entry.Relative + [string][char]0 + $entry.FileLength.ToString() + [string][char]0 + $entry.Ticks.ToString() + "`n"
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($line)
        [void]$sha.TransformBlock($bytes, 0, $bytes.Length, $bytes, 0)
    }}
    [void]$sha.TransformFinalBlock([byte[]]@(), 0, 0)
    $digest = [System.BitConverter]::ToString($sha.Hash).Replace('-', '').ToLowerInvariant()
}} finally {{
    $sha.Dispose()
}}
$answer = [string]::Join("`n", @($total.ToString(), $digest, $base, ([long]$skipped).ToString(), $files.ToString())) + "`n"
$answerBytes = (New-Object System.Text.UTF8Encoding $false).GetBytes($answer)
$stdout = [Console]::OpenStandardOutput()
$stdout.Write($answerBytes, 0, $answerBytes.Length)
$stdout.Flush()"#,
        path = path_expression(path),
    )
}

/// The PowerShell script that writes a zip of the folder to stdout. The zip
/// is built in a temporary file, so the host's own output cap sees nothing
/// until the archive is complete; it refuses one over `max_bytes`.
pub(super) fn powershell_archive_script(path: &str, max_bytes: u64) -> String {
    // .NET's zip length and PowerShell's arithmetic are signed; no real limit
    // comes near it.
    let max = max_bytes.min(i64::MAX as u64);
    format!(
        r#"{PREAMBLE}
$path = {path}
if (-not [System.IO.Directory]::Exists($path)) {{
    if ([System.IO.File]::Exists($path)) {{ exit {NOT_A_FILE_EXIT} }}
    exit {MISSING_EXIT}
}}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$root = [System.IO.Path]::GetFullPath($path).TrimEnd([char[]]@('\', '/'))
if ($root -eq '') {{ $root = '/' }}
$base = [System.IO.Path]::GetFileName($root)
if ($base -eq '') {{ exit {NOT_A_FILE_EXIT} }}
$prefix = $root
if ($root -ne '/') {{ $prefix = $root + [System.IO.Path]::DirectorySeparatorChar }}
$zip = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), [System.IO.Path]::GetRandomFileName() + '.zip')
try {{
    $archive = [System.IO.Compression.ZipFile]::Open($zip, 'Create')
    try {{
        $directories = New-Object System.Collections.Stack
        $directories.Push($path)
        while ($directories.Count -gt 0) {{
            $directory = $directories.Pop()
            $normalized = [System.IO.Path]::GetFullPath($directory).TrimEnd([char[]]@('\', '/'))
            if ($normalized -ne $root) {{
                $relative = $normalized.Substring($prefix.Length).Replace('\', '/')
                [void]$archive.CreateEntry($base + '/' + $relative + '/')
            }}
            foreach ($item in [System.IO.Directory]::GetFileSystemEntries($directory)) {{
                $attributes = [System.IO.File]::GetAttributes($item)
                if (($attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {{ continue }}
                if (($attributes -band [System.IO.FileAttributes]::Directory) -ne 0) {{
                    $directories.Push($item)
                }} else {{
                    $relative = [System.IO.Path]::GetFullPath($item).Substring($prefix.Length).Replace('\', '/')
                    [void][System.IO.Compression.ZipFileExtensions]::CreateEntryFromFile($archive, $item, $base + '/' + $relative)
                }}
            }}
        }}
    }} finally {{
        $archive.Dispose()
    }}
    $file = [System.IO.File]::OpenRead($zip)
    try {{
        if ($file.Length -gt {max}) {{ exit {TOO_LARGE_EXIT} }}
        $stdout = [Console]::OpenStandardOutput()
        $file.CopyTo($stdout)
        $stdout.Flush()
    }} finally {{
        $file.Dispose()
    }}
}} finally {{
    Remove-Item -LiteralPath $zip -Force -ErrorAction SilentlyContinue
}}
exit 0"#,
        path = path_expression(path),
    )
}
