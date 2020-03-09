use anyhow::{anyhow, Result};
use pico_args::Arguments;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    thread::spawn,
};

fn npm() -> String {
    env::var("NPM").unwrap_or_else(|_| "npm".to_string())
}

fn run_npm() -> Result<()> {
    let npm = npm();
    let status = Command::new(npm)
        .current_dir(frontend_dir())
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

fn run_cargo_watch_rsbt_web() -> Result<()> {
    let cargo = cargo();
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(&["watch", "-x", "run --bin rsbt-web"])
        .status()?;

    if !status.success() {
        return Err(anyhow!("'cargo watch' failed"));
    }

    Ok(())
}

fn cargo_build() -> Result<()> {
    let cargo = cargo();
    let status = Command::new(cargo)
        .current_dir(project_root())
        .arg("build")
        .status()?;

    if !status.success() {
        return Err(anyhow!("'cargo build' failed"));
    }

    Ok(())
}

fn npm_install() -> Result<()> {
    let npm = npm();
    let status = Command::new(npm)
        .current_dir(frontend_dir())
        .arg("install")
        .status()?;

    if !status.success() {
        return Err(anyhow!("'npm install' failed"));
    }

    Ok(())
}

fn dir_clean<P: AsRef<Path>>(p: P) -> Result<()> {
    if p.as_ref().exists() {
        std::fs::remove_dir_all(p)?;
    }

    Ok(())
}

fn npm_clean() -> Result<()> {
    dir_clean(frontend_dir().join("node_modules"))
}

fn www_target_clean() -> Result<()> {
    dir_clean(frontend_dir().join("target"))
}

fn cargo_clean() -> Result<()> {
    dir_clean(project_root().join("target"))
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

    if !status.success() {
        return Err(anyhow!("'cargo install watch' failed"));
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

fn frontend_dir() -> PathBuf {
    project_root().join("rsbt-web-resources/www")
}

fn main() -> Result<()> {
    let mut args = Arguments::from_env();
    let subcommand = args.subcommand()?.unwrap_or_default();

    match subcommand.as_str() {
        "clean" => {
            args.finish()?;
            cargo_clean()?;
            npm_clean()?;
            www_target_clean()?;
        }
        "dev" => {
            args.finish()?;

            if let Err(err) = spawn(cargo_build).join() {
                eprintln!("Cannot build, ignore for now: {:?}", err);
            }

            let npm_task = spawn(run_npm);

            let cargo_task = spawn(run_cargo_watch_rsbt_web);

            npm_task.join().expect("cannot join npm")?;
            cargo_task.join().expect("cannot join cargo")?;
        }
        "install" => {
            args.finish()?;
            install_cargo_watch()?;
            npm_install()?;
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
    clean
    dev
    install
    ui-dev"
            );
        }
    }

    Ok(())
}
