use anyhow::{anyhow, Result};
use pico_args::Arguments;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    thread::spawn,
};

fn run_npm() -> Result<()> {
    let npm = env::var("NPM").unwrap_or_else(|_| "npm".to_string());
    let status = Command::new(npm)
        .current_dir(project_root().join("rustorrent-web-resources/www"))
        .args(&["run", "start"])
        .status()?;

    if !status.success() {
        return Err(anyhow!("'npm run start' failed"));
    }

    Ok(())
}

fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn run_cargo_watch_rustorrent_web() -> Result<()> {
    let cargo = cargo();
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(&["watch", "-x", "run --bin rustorrent-web"])
        .status()?;

    if !status.success() {
        return Err(anyhow!("'cargo watch' failed"));
    }

    Ok(())
}

fn install_cargo_watch() -> Result<()> {
    let cargo_cmd = cargo();
    let status = Command::new(cargo_cmd)
        .args(&["watch", "--version"])
        .status()?;

    if status.success() {
        return Ok(());
    }

    let status = Command::new(cargo()).args(&["install", "watch"]).status()?;

    if status.success() {
        return Ok(());
    }

    Ok(())
}

fn main() -> Result<()> {
    let mut args = Arguments::from_env();
    let subcommand = args.subcommand()?.unwrap_or_default();

    match subcommand.as_str() {
        "dev" => {
            args.finish()?;
            let npm_task = spawn(run_npm);

            let cargo_task = spawn(run_cargo_watch_rustorrent_web);

            npm_task.join().expect("cannot join npm")?;
            cargo_task.join().expect("cannot join cargo")?;
        }
        "install" => {
            args.finish()?;
            install_cargo_watch()?;
        }
        "ui-dev" => {
            args.finish()?;
            run_npm()?;
        }
        _ => {
            eprintln!(
                "\
cargo xtask
Run custom build command.

USAGE:
    cargo xtask <SUBCOMMAND>

SUBCOMMANDS:
    dev
    install
    ui-dev"
            );
        }
    }

    Ok(())
}

pub fn project_root() -> PathBuf {
    Path::new(
        &env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned()),
    )
    .ancestors()
    .nth(1)
    .unwrap()
    .to_path_buf()
}
