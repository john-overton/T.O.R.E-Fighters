//! `--import FOLDER`: the game's own import, run without the game. Writes the
//! import into the data folder and says where its report is.

use std::path::Path;
use tore_import::{Progress, check_markers, import_with_progress, media_source::MediaSource};

/// Imports the game at `folder` into `data_dir`, printing progress with
/// `say`, and returns the lines to print at the end.
pub fn import(
    folder: &Path,
    data_dir: &Path,
    say: &mut dyn FnMut(&str),
) -> Result<Vec<String>, String> {
    let source = MediaSource::detect(folder).map_err(|error| error.to_string())?;
    say(&format!(
        "Importing Fighters Anthology from {} ({})",
        source.path.display(),
        source.kind.label()
    ));
    let mut last_archive = String::new();
    let imported = import_with_progress(
        &source,
        data_dir,
        &mut |progress| match progress {
            Progress::Preparing(text) => say(&format!("{text}...")),
            Progress::Reading {
                archive,
                done,
                total,
            } => {
                if archive != last_archive {
                    say(&format!("Reading {archive}"));
                    last_archive = archive;
                }
                if let Some(total) = total
                    && done == total
                {
                    say(&format!("  {total} resources"));
                }
            }
        },
        // The server needs only the simulation's data; the game checks its
        // menu art itself when it loads the same pack.
        &|resources| check_markers(resources),
    )
    .map_err(|error| format!("The import failed: {error}"))?;
    let mut lines = imported.summary;
    lines.push(format!("Imported into {}", data_dir.display()));
    lines.push(format!(
        "The import report is {}",
        data_dir.join("import-report.txt").display()
    ));
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log::tests::scratch;
    use std::fs;

    #[test]
    fn a_folder_that_is_not_the_game_is_refused_in_plain_words() {
        let dir = scratch("import-not");
        fs::create_dir_all(&dir).unwrap();
        let error = import(&dir, &dir.join("data"), &mut |_| {}).unwrap_err();
        assert!(
            error.contains("is not a Fighters Anthology source"),
            "{error}"
        );
        let error = import(&dir.join("absent"), &dir.join("data"), &mut |_| {}).unwrap_err();
        assert!(error.contains("does not exist"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }
}
