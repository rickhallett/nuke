mod apps;
mod config;
mod menu;
mod plan;

use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};

use apps::{App, Policy};
use plan::Plan;

/// Quit all running macOS applications at once.
///
/// By default every app with a Dock icon is asked to quit gracefully, except
/// Finder, the terminal you ran this from, and anything listed under `keep`
/// in ~/.config/nuke/config.toml. Apps with unsaved changes will show their
/// own save dialog and stay open until it is answered.
#[derive(Parser, Debug)]
#[command(version, about, verbatim_doc_comment)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// List running apps and exit, without quitting anything.
    #[arg(short, long)]
    list: bool,

    /// Show what would be quit, but do not quit anything.
    #[arg(short = 'n', long)]
    dry_run: bool,

    /// Additional apps to leave running (name or bundle id). Repeatable or comma-separated.
    #[arg(
        short = 'x',
        long = "except",
        value_name = "APP",
        value_delimiter = ','
    )]
    except: Vec<String>,

    /// Quit only these apps (name or bundle id). Overrides the keep list.
    #[arg(
        short,
        long,
        value_name = "APP",
        value_delimiter = ',',
        conflicts_with = "except"
    )]
    only: Vec<String>,

    /// Also quit menu-bar / accessory apps (those without a Dock icon).
    #[arg(short = 'a', long)]
    include_accessory: bool,

    /// Force-quit immediately instead of asking politely. Unsaved work is lost.
    #[arg(short, long)]
    force: bool,

    /// Seconds to wait for apps to exit before reporting them as still running.
    #[arg(short, long, value_name = "SECS")]
    timeout: Option<u64>,

    /// Force-quit anything still running after SECS seconds of graceful waiting.
    #[arg(long, value_name = "SECS", conflicts_with = "force")]
    force_after: Option<u64>,

    /// Don't wait for apps to exit; send the quit request and return.
    #[arg(long, conflicts_with_all = ["timeout", "force_after"])]
    no_wait: bool,

    /// Only print errors.
    #[arg(short, long)]
    quiet: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run as a menu bar app: a status-bar icon with a Quit All button and a
    /// tickable list of apps to keep. Shares the CLI's config file.
    Menu,
}

fn main() -> ExitCode {
    // Double-clicked as Nuke.app: no arguments, living inside a bundle.
    // That's the muggle entry point; go straight to the menu bar.
    let cli = if launched_from_bundle() {
        Cli::parse_from(["nuke", "menu"])
    } else {
        Cli::parse()
    };
    let cfg = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nuke: bad config: {e}");
            return ExitCode::from(2);
        }
    };

    if let Some(Command::Menu) = cli.command {
        menu::run(cfg);
        return ExitCode::SUCCESS;
    }

    let all = apps::running();
    let ancestors = apps::ancestor_pids();

    if cli.list {
        list(&all, &ancestors);
        return ExitCode::SUCCESS;
    }

    let keep: Vec<&str> = cfg
        .keep
        .iter()
        .chain(cli.except.iter())
        .map(String::as_str)
        .collect();
    let plan = Plan {
        keep,
        only: &cli.only,
        include_accessory: cli.include_accessory || cfg.include_accessory,
        ancestors: &ancestors,
    };
    let targets: Vec<&App> = all.iter().filter(|a| plan.targets(a)).collect();

    if !cli.only.is_empty() {
        for o in &cli.only {
            if !all.iter().any(|a| a.matches(o)) {
                eprintln!("nuke: {o}: not running");
            }
        }
    }

    if targets.is_empty() {
        if !cli.quiet {
            println!("Nothing to quit.");
        }
        return ExitCode::SUCCESS;
    }

    let verb = if cli.force {
        "Force quitting"
    } else {
        "Quitting"
    };
    for app in &targets {
        if !cli.quiet || cli.dry_run {
            println!("{verb} {}", describe(app));
        }
    }
    if cli.dry_run {
        return ExitCode::SUCCESS;
    }

    // Send the requests. terminate()/forceTerminate() return immediately; they
    // only tell us whether the request was delivered.
    let mut sent: Vec<&App> = Vec::new();
    let mut failed = 0;
    for app in &targets {
        let ok = if cli.force {
            app.force_terminate()
        } else {
            app.terminate()
        };
        if ok {
            sent.push(app);
        } else {
            failed += 1;
            eprintln!("nuke: could not send quit to {}", describe(app));
        }
    }

    if cli.no_wait {
        return exit_status(failed);
    }

    let timeout = Duration::from_secs(cli.timeout.or(cfg.timeout).unwrap_or(5));
    let force_after = cli.force_after.or(cfg.force_after);
    let wait = force_after.map_or(timeout, Duration::from_secs);
    let mut alive = apps::wait_for_exit(&sent, wait);

    if !alive.is_empty() && force_after.is_some() && !cli.force {
        for app in &alive {
            if !cli.quiet {
                println!(
                    "Force quitting {} (still running after {}s)",
                    describe(app),
                    wait.as_secs()
                );
            }
            if !app.force_terminate() {
                eprintln!("nuke: could not force quit {}", describe(app));
            }
        }
        alive = apps::wait_for_exit(&alive, timeout);
    }

    for app in &alive {
        eprintln!(
            "nuke: {} is still running (unsaved changes?)",
            describe(app)
        );
    }
    failed += alive.len();

    if !cli.quiet {
        let quit = sent.len() - alive.len();
        println!(
            "Quit {quit} of {} app{}.",
            targets.len(),
            if targets.len() == 1 { "" } else { "s" }
        );
    }
    exit_status(failed)
}

fn launched_from_bundle() -> bool {
    std::env::args_os().len() == 1
        && std::env::current_exe()
            .map(|p| p.components().any(|c| c.as_os_str() == "MacOS"))
            .unwrap_or(false)
}

fn list(all: &[App], ancestors: &std::collections::HashSet<i32>) {
    let width = all
        .iter()
        .map(|a| a.name.chars().count())
        .max()
        .unwrap_or(0);
    for app in all.iter().filter(|a| a.policy != Policy::Prohibited) {
        let note = if ancestors.contains(&app.pid) {
            "  (host terminal, protected)"
        } else {
            ""
        };
        println!(
            "{:<width$}  {:<9}  {:>6}  {}{note}",
            app.name,
            app.policy.label(),
            app.pid,
            app.bundle_id.as_deref().unwrap_or("-"),
        );
    }
}

fn describe(app: &App) -> String {
    match &app.bundle_id {
        Some(id) => format!("{} ({id}, pid {})", app.name, app.pid),
        None => format!("{} (pid {})", app.name, app.pid),
    }
}

fn exit_status(failed: usize) -> ExitCode {
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
