//! zman: command line interface to the data recorded by zman-daemon.

mod live;
mod reports;
mod ui;

use clap::{Parser, Subcommand};
use zman_core::Lang;

#[derive(Parser)]
#[command(name = "zman", version, about = "Automatic, local-first time tracker")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Summary of a day: hours, categories and top apps
    Today {
        /// Day to show instead of today (YYYY-MM-DD)
        #[arg(long, value_name = "DATE")]
        date: Option<String>,
    },
    /// Summary of a week, Monday to Sunday
    Week {
        /// Any day of the week to show instead of the current one (YYYY-MM-DD)
        #[arg(long, value_name = "DATE")]
        date: Option<String>,
    },
    /// Time per app for a day, or a week with --week
    Apps {
        /// Day (or any day of the week) to show instead of today (YYYY-MM-DD)
        #[arg(long, value_name = "DATE")]
        date: Option<String>,
        /// Show the whole week instead of one day
        #[arg(long)]
        week: bool,
        /// Maximum number of apps to list
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// Follow what the daemon is recording, live
    Watch,
    /// Show file locations, device id and whether the daemon is running
    Status,
}

fn main() {
    let cli = Cli::parse();
    let lang = Lang::detect();
    let result = match cli.command {
        Command::Today { date } => reports::today(lang, date.as_deref()),
        Command::Week { date } => reports::week(lang, date.as_deref()),
        Command::Apps { date, week, limit } => reports::apps(lang, date.as_deref(), week, limit),
        Command::Watch => live::watch(lang),
        Command::Status => live::status(lang),
    };
    if let Err(e) = result {
        eprintln!("zman: {e}");
        std::process::exit(1);
    }
}
