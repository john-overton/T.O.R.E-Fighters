//! The server's log: `logs/server-<date>.log` in the data folder, one line
//! each with a UTC time, and the same lines without the time on the console.
//! The date is UTC because the standard library has no time zones (agent
//! decision); a new file starts at UTC midnight.

use crate::clock::utc_stamp;
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

/// The log file and the console it echoes to.
pub struct Logger {
    folder: PathBuf,
    current: Option<(String, File)>,
    echo: Box<dyn Write>,
    complained: bool,
}

impl Logger {
    /// A logger writing into `folder` and echoing to `echo` (standard output).
    pub fn new(folder: PathBuf, echo: Box<dyn Write>) -> Self {
        Self {
            folder,
            current: None,
            echo,
            complained: false,
        }
    }

    /// The file for a date, `server-YYYY-MM-DD.log`.
    pub fn file_name(date: &str) -> String {
        format!("server-{date}.log")
    }

    /// Records one line in the log and on the console.
    pub fn log(&mut self, unix_seconds: u64, text: &str) {
        let (date, time) = utc_stamp(unix_seconds);
        for line in text.lines() {
            self.write_file(&date, &format!("{date} {time} {line}\n"));
            self.print(line);
        }
    }

    /// Records one line in the log only.
    pub fn log_file_only(&mut self, unix_seconds: u64, text: &str) {
        let (date, time) = utc_stamp(unix_seconds);
        for line in text.lines() {
            self.write_file(&date, &format!("{date} {time} {line}\n"));
        }
    }

    /// Writes one line to the console only: a status line or a command's answer.
    pub fn print(&mut self, text: &str) {
        let _ = writeln!(self.echo, "{text}");
        let _ = self.echo.flush();
    }

    fn write_file(&mut self, date: &str, line: &str) {
        if self.current.as_ref().is_none_or(|(open, _)| open != date) {
            match self.open(date) {
                Ok(file) => self.current = Some((date.to_owned(), file)),
                Err(error) => {
                    self.current = None;
                    if !self.complained {
                        self.complained = true;
                        let _ = writeln!(
                            self.echo,
                            "The log file in {} cannot be written: {error}. Carrying on without it.",
                            self.folder.display()
                        );
                    }
                    return;
                }
            }
        }
        if let Some((_, file)) = &mut self.current {
            // A failed write must never stop the server.
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn open(&self, date: &str) -> std::io::Result<File> {
        fs::create_dir_all(&self.folder)?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.folder.join(Self::file_name(date)))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    /// A writer the test can read back.
    #[derive(Clone, Default)]
    pub struct Shared(pub Rc<RefCell<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Shared {
        pub fn text(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).unwrap()
        }
    }

    pub fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("TMPDIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let path = base.join(format!("tore-server-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn a_line_goes_to_the_dated_file_and_the_console() {
        let folder = scratch("log-basic").join("logs");
        let console = Shared::default();
        let mut log = Logger::new(folder.clone(), Box::new(console.clone()));
        log.log(1_790_771_696, "Waiting for players");
        log.print("status line");
        assert_eq!(console.text(), "Waiting for players\nstatus line\n");
        let file = fs::read_to_string(folder.join("server-2026-09-30.log")).unwrap();
        assert_eq!(file, "2026-09-30 12:34:56 Waiting for players\n");
        let _ = fs::remove_dir_all(folder.parent().unwrap());
    }

    #[test]
    fn a_new_day_starts_a_new_file_and_a_file_only_line_skips_the_console() {
        let folder = scratch("log-days").join("logs");
        let console = Shared::default();
        let mut log = Logger::new(folder.clone(), Box::new(console.clone()));
        log.log(1_798_761_599, "last second");
        log.log_file_only(1_798_761_600, "first second");
        assert_eq!(console.text(), "last second\n");
        assert_eq!(
            fs::read_to_string(folder.join("server-2026-12-31.log")).unwrap(),
            "2026-12-31 23:59:59 last second\n"
        );
        assert_eq!(
            fs::read_to_string(folder.join("server-2027-01-01.log")).unwrap(),
            "2027-01-01 00:00:00 first second\n"
        );
        let _ = fs::remove_dir_all(folder.parent().unwrap());
    }

    #[test]
    fn an_unwritable_folder_is_reported_once_and_does_not_stop_the_log() {
        let root = scratch("log-blocked");
        fs::create_dir_all(&root).unwrap();
        let blocker = root.join("logs");
        fs::write(&blocker, "a file, not a folder").unwrap();
        let console = Shared::default();
        let mut log = Logger::new(blocker, Box::new(console.clone()));
        log.log(0, "one");
        log.log(0, "two");
        let text = console.text();
        assert_eq!(text.matches("cannot be written").count(), 1, "{text}");
        assert!(text.contains("one\n") && text.contains("two\n"));
        let _ = fs::remove_dir_all(root);
    }
}
