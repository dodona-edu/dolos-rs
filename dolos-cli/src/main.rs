use crate::opts::{Command, Opts, OutputFormat};
use clap::Parser;
use dolos::Dolos;
use std::process::ExitCode;

mod logging;
mod opts;
mod views;

fn main() -> ExitCode {
    let opts = Opts::parse();
    logging::init(opts.verbosity.log_level_filter(), step_count(&opts.command));

    match run(opts.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e:#}");
            ExitCode::FAILURE
        }
    }
}

/// Run `command`.
fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Run { files, dolos_args, output_args } => {
            let report = Dolos::new(files, dolos_args.try_into()?)?.build_report();
            views::show(output_args, &report)?;
        }
    }
    Ok(())
}

/// The number of steps that `command` reports: read, fingerprint and compare,
/// plus the template and the report directory when they apply.
fn step_count(command: &Command) -> usize {
    match command {
        Command::Run { dolos_args, output_args, .. } => {
            let template = usize::from(dolos_args.ignore.is_some());
            let report_directory = usize::from(!matches!(
                output_args.output_format,
                OutputFormat::Terminal | OutputFormat::Console
            ));
            3 + template + report_directory
        }
    }
}
