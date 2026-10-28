//! The logs of Downloads in the app data folder: everything yt-dlp printed
//! while receiving one, so that a Failed one can be looked into. Only the
//! newest few are kept.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::SystemTime;

/// How many logs are kept; the oldest go first.
const KEPT: usize = 10;

/// How many of the last lines yt-dlp printed a Download keeps for the user
/// to copy.
const LAST_LINES: usize = 20;

/// The folder of the logs, one per Download received.
pub struct Logs {
    dir: PathBuf,
}

impl Logs {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Starts the log of a Download of the Video with the site's id `id`,
    /// named by the time `now` it starts, and removes the oldest logs, so
    /// that only the [`KEPT`] newest stay with it. A log that can't be
    /// written keeps only the last lines.
    pub fn start(&self, now: SystemTime, id: &str) -> Log {
        // Created before the oldest are removed, so that it never takes the
        // name of one of them and passes for older than it is.
        let file = fs::create_dir_all(&self.dir).and_then(|()| {
            let file = self.create(&format!("{} {}", timestamp(now), file_safe(id)));
            self.remove_oldest(KEPT);
            file
        });
        let file = file
            .inspect_err(|error| {
                eprintln!("can't start a log in {}: {error}", self.dir.display());
            })
            .ok();
        Log {
            file,
            last_lines: VecDeque::new(),
        }
    }

    /// Removes the oldest logs, so that only the `kept` newest stay: the
    /// ones last written to first, since a Download writes its log until it
    /// ends, and the next one starts after; then by name, which begins with
    /// the time it started.
    fn remove_oldest(&self, kept: usize) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut logs: Vec<(Option<SystemTime>, PathBuf)> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
            .map(|path| {
                let modified = fs::metadata(&path).and_then(|it| it.modified()).ok();
                (modified, path)
            })
            .collect();
        logs.sort();
        for (_, log) in &logs[..logs.len().saturating_sub(kept)] {
            if let Err(error) = fs::remove_file(log) {
                eprintln!("can't remove the log {}: {error}", log.display());
            }
        }
    }

    /// Creates the first free file of `<name>.log`, `<name> (1).log`…
    fn create(&self, name: &str) -> io::Result<File> {
        for number in 0.. {
            let name = match number {
                0 => format!("{name}.log"),
                _ => format!("{name} ({number}).log"),
            };
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.dir.join(name))
            {
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                file => return file,
            }
        }
        unreachable!("some number is free")
    }
}

/// The log of one Download.
pub struct Log {
    /// `None` once it can't be written.
    file: Option<File>,
    last_lines: VecDeque<String>,
}

impl Log {
    /// Writes a line of the app's own, such as how yt-dlp was run.
    pub fn note(&mut self, line: &str) {
        if let Some(file) = &mut self.file
            && let Err(error) = writeln!(file, "{line}")
        {
            eprintln!("can't write a log: {error}");
            self.file = None;
        }
    }

    /// Writes a line yt-dlp printed, and keeps it among the last ones.
    pub fn output(&mut self, line: &str) {
        self.note(line);
        if self.last_lines.len() == LAST_LINES {
            self.last_lines.pop_front();
        }
        self.last_lines.push_back(line.to_owned());
    }

    /// The last lines yt-dlp printed, oldest first.
    pub fn last_lines(&self) -> Vec<String> {
        self.last_lines.iter().cloned().collect()
    }
}

/// `YYYY-MM-DD HH-MM-SS` of `time` in UTC, which sorts as the time does.
fn timestamp(time: SystemTime) -> String {
    let seconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let (days, second) = (seconds / 86_400, seconds % 86_400);
    let (year, month, day) = date(days);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}-{:02}-{:02}",
        second / 3600,
        second / 60 % 60,
        second % 60
    )
}

/// The year, month and day `days` after 1970-01-01, in the Gregorian
/// calendar (Howard Hinnant's `civil_from_days`).
fn date(days: u64) -> (u64, u64, u64) {
    // Counted from 0000-03-01, so that a leap day ends its year.
    let days = days + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    (year, month, day)
}

/// The site's id of a Video with what can't be in a file name replaced.
fn file_safe(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
