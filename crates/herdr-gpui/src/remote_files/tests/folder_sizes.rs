//! How the folder question writes a tree's size and file count.

use crate::remote_files::{file_count, folder_size};

#[test]
fn a_folder_size_takes_the_unit_that_fits() {
    let cases = [
        (0, "0 KB"),
        (1, "1 KB"),
        (4_096, "4 KB"),
        (831_488, "812 KB"),
        (1_048_575, "1.0 MB"),
        (1_048_576, "1.0 MB"),
        (13_002_342, "12.4 MB"),
        (1_073_741_823, "1.0 GB"),
        (1_073_741_824, "1.0 GB"),
        (1_395_864_371, "1.3 GB"),
    ];
    let wrong: Vec<_> = cases
        .iter()
        .map(|&(bytes, expected)| (bytes, folder_size(bytes), expected))
        .filter(|(_, shown, expected)| shown != expected)
        .collect();
    assert!(wrong.is_empty(), "(bytes, shown, expected): {wrong:?}");
}

#[test]
fn a_file_count_groups_thousands_and_names_one_file() {
    for (count, expected) in [
        (0, "0 files"),
        (1, "1 file"),
        (342, "342 files"),
        (18_204, "18,204 files"),
        (1_234_567, "1,234,567 files"),
    ] {
        assert_eq!(file_count(count), expected, "{count} files");
    }
}
