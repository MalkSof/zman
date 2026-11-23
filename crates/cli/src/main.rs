//! zman: command line interface to the data recorded by zman-daemon.

mod live;
mod reports;
mod sync;
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
    /// Export the recorded events as CSV
    Export {
        /// First day (YYYY-MM-DD). Defaults to today
        #[arg(long, value_name = "DATE")]
        from: Option<String>,
        /// Last day, inclusive (YYYY-MM-DD). Defaults to --from
        #[arg(long, value_name = "DATE")]
        to: Option<String>,
        /// Write to this file instead of the terminal
        #[arg(long, short, value_name = "FILE")]
        output: Option<std::path::PathBuf>,
    },
    /// Upload this device's events to a zman server (all your devices in one place)
    Sync {
        #[command(subcommand)]
        action: Option<SyncAction>,
    },
    /// Follow what the daemon is recording, live
    Watch,
    /// Show file locations, device id and whether the daemon is running
    Status,
}

#[derive(Subcommand)]
enum SyncAction {
    /// Sign this device in to a server
    Login {
        /// Server address, e.g. https://zman.example.com
        server: String,
        /// Account email (asked for if omitted)
        #[arg(long)]
        email: Option<String>,
        /// Create the account first
        #[arg(long)]
        register: bool,
    },
    /// Upload new events now (the default action)
    Now,
    /// Show the account and when the last sync happened
    Status,
    /// Sign this device out; local data is kept
    Logout,
}

fn main() {
    let cli = Cli::parse();
    let lang = Lang::detect();
    let result = match cli.command {
        Command::Today { date } => reports::today(lang, date.as_deref()),
        Command::Week { date } => reports::week(lang, date.as_deref()),
        Command::Apps { date, week, limit } => reports::apps(lang, date.as_deref(), week, limit),
        Command::Export { from, to, output } => reports::export(lang, from.as_deref(), to.as_deref(), output.as_deref()),
        Command::Sync { action } => match action {
            Some(SyncAction::Login { server, email, register }) => sync::login(lang, &server, email, register),
            Some(SyncAction::Now) | None => sync::run_now(lang),
            Some(SyncAction::Status) => sync::status(lang),
            Some(SyncAction::Logout) => sync::logout(lang),
        },
        Command::Watch => live::watch(lang),
        Command::Status => live::status(lang),
    };
    if let Err(e) = result {
        eprintln!("zman: {e}");
        std::process::exit(1);
    }
}
