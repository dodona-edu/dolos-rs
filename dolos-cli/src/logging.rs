use anstyle::{AnsiColor, Effects, Style};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::io::{Result, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Print the log records of the `dolos` crates on stderr, from `level` up.
///
/// A record with a `step` key prints as `[n/step_count] message`, a warning as
/// `warning: message` and an error as `error: message`. Errors always print.
pub fn init(level: LevelFilter, step_count: usize) {
    let level = level.max(LevelFilter::Error);
    let logger = CliLogger { level, step_count, step: AtomicUsize::new(0) };
    log::set_boxed_logger(Box::new(logger)).expect("the logger is set only once");
    log::set_max_level(level);
}

struct CliLogger {
    level: LevelFilter,
    step_count: usize,
    step: AtomicUsize,
}

impl Log for CliLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        let target = metadata.target();
        metadata.level() <= self.level && (target == "dolos" || target.starts_with("dolos::"))
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // A failed write to stderr cannot be reported, so it is ignored.
            let _ = self.write(&mut anstream::stderr().lock(), record);
        }
    }

    fn flush(&self) {}
}

impl CliLogger {
    fn write(&self, out: &mut impl Write, record: &Record) -> Result<()> {
        if record.key_values().get("step".into()).is_some() {
            let n = self.step.fetch_add(1, Ordering::Relaxed) + 1;
            return writeln!(out, "[{n}/{}] {}", self.step_count, record.args());
        }

        let (prefix, color) = match record.level() {
            Level::Info => return writeln!(out, "{}", record.args()),
            Level::Warn => ("warning", AnsiColor::Yellow),
            Level::Error => ("error", AnsiColor::Red),
            Level::Debug => ("debug", AnsiColor::Blue),
            Level::Trace => ("trace", AnsiColor::Cyan),
        };
        let style = Style::new()
            .fg_color(Some(color.into()))
            .effects(Effects::BOLD);
        writeln!(out, "{style}{prefix}:{style:#} {}", record.args())
    }
}
