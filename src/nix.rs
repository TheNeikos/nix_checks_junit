use std::{collections::HashMap, process::Stdio};

use crate::nix::BuildMode::DryRun;
use camino::{Utf8Path, Utf8PathBuf};

#[tracing::instrument(level = "debug", err)]
pub async fn show() -> anyhow::Result<serde_json::Value> {
    let cmd = tokio::process::Command::new("nix")
        .args(["flake", "show", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !cmd.status.success() {
        return Err(anyhow::anyhow!(
            "`nix flake show --json` did not run succesfully.\nStdout:{}\nStderr:{}",
            String::from_utf8_lossy(&cmd.stdout),
            String::from_utf8_lossy(&cmd.stderr)
        ));
    }

    Ok(serde_json::from_slice(&cmd.stdout)?)
}

#[tracing::instrument(level = "debug", err)]
pub(crate) async fn current_system() -> anyhow::Result<String> {
    let cmd = tokio::process::Command::new("nix")
        .args([
            "eval",
            "--impure",
            "--raw",
            "--expr",
            "builtins.currentSystem",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !cmd.status.success() {
        return Err(anyhow::anyhow!(
            "`nix eval --impure --raw --expr 'builtins.currentSystem'` did not run succesfully.\nStdout:{}\nStderr:{}",
            String::from_utf8_lossy(&cmd.stdout),
            String::from_utf8_lossy(&cmd.stderr)
        ));
    }

    Ok(String::from_utf8(cmd.stdout)?)
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct BuildDerivation {
    #[serde(rename = "drvPath")]
    pub(crate) drv_path: Utf8PathBuf,
    #[allow(dead_code)]
    outputs: HashMap<String, Utf8PathBuf>,
}

#[derive(Debug, PartialEq)]
pub enum BuildMode {
    DryRun,
    Real,
}

#[tracing::instrument(level = "debug")]
pub(crate) async fn build(
    build_target: String,
    build_mode: BuildMode,
    nix_options: &Vec<String>,
) -> anyhow::Result<Vec<BuildDerivation>> {
    let mut args = vec!["build", build_target.as_str(), "--json"];
    if build_mode == DryRun {
        args.push("--dry-run");
    }
    nix_options.iter().for_each(|o| args.push(o.as_str()));
    tracing::debug!("running nix {:?}", args);

    let cmd = tokio::process::Command::new("nix")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !cmd.status.success() {
        return Err(anyhow::anyhow!(
            "`nix build {build_target} --json` did not run succesfully.\nStdout:{}\nStderr:{}",
            String::from_utf8_lossy(&cmd.stdout),
            String::from_utf8_lossy(&cmd.stderr)
        ));
    }

    Ok(serde_json::from_slice(&cmd.stdout)?)
}

#[tracing::instrument(level = "debug")]
pub(crate) async fn log(drv_path: &Utf8Path) -> anyhow::Result<String> {
    let cmd = tokio::process::Command::new("nix")
        .args(["log", drv_path.as_str()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if !cmd.status.success() {
        return Err(anyhow::anyhow!(
            "`nix log {drv_path}` did not run succesfully.\nStdout:{}\nStderr:{}",
            String::from_utf8_lossy(&cmd.stdout),
            String::from_utf8_lossy(&cmd.stderr)
        ));
    }

    Ok(String::from_utf8(cmd.stdout)?)
}
