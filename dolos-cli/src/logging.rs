use log::{Level, LevelFilter};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Print the log records of the `dolos` crates on stderr, from `level` up.
///
/// A record with a `step` key prints as `[n/step_count] message`, a warning as
/// `warning: message` and an error as `error: message`. Errors always print.
pub fn init(level: LevelFilter, step_count: usize) {
    let step = AtomicUsize::new(0);
    env_logger::Builder::new()
        .filter_module("dolos", level.max(LevelFilter::Error))
        .parse_default_env()
        .format(move |buf, record| {
            if record.key_values().get("step".into()).is_some() {
                let n = step.fetch_add(1, Ordering::Relaxed) + 1;
                return writeln!(buf, "[{n}/{step_count}] {}", record.args());
            }

            let style = buf.default_level_style(record.level());
            let prefix = match record.level() {
                Level::Info => return writeln!(buf, "{}", record.args()),
                Level::Warn => "warning",
                Level::Error => "error",
                Level::Debug => "debug",
                Level::Trace => "trace",
            };
            writeln!(buf, "{style}{prefix}:{style:#} {}", record.args())
        })
        .init();
}
