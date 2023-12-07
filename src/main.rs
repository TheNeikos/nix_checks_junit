use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

mod nix;

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use junit_report::{ReportBuilder, TestCase, TestCaseBuilder, TestSuiteBuilder};
use tracing::{debug, error, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Debug, clap::Parser)]
#[clap(
    name = clap::crate_name!(),
    version = clap::crate_version!(),
    about = clap::crate_description!()
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    RunChecks {
        /// The path where the output should be written to
        #[clap(short, long, value_enum)]
        output_path: Utf8PathBuf,

        /// The number of --max-jobs to pass to nix
        #[clap(long)]
        max_jobs: Option<NonZeroUsize>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::filter::EnvFilter::builder()
        .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
        .from_env_lossy();
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .pretty();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    let args = Cli::parse();
    debug!(?args, "Running app with args");

    match args.command {
        Command::RunChecks {
            output_path,
            max_jobs,
        } => {
            run_checks(&output_path, max_jobs).await?;
        }
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct Derivation {
    name: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    kind: monostate::MustBe!("derivation"),
}

enum CheckResult {
    Success,
    Failure { log_output: String },
}

struct CheckTestCase {
    name: String,
    result: CheckResult,
    duration: Duration,
}

async fn run_checks(output_path: &Utf8Path, max_jobs: Option<NonZeroUsize>) -> anyhow::Result<()> {
    let checks_structure = crate::nix::show().await?;
    debug!(?checks_structure, "Got checks structure");

    let current_system = crate::nix::current_system().await?;
    debug!(?current_system, "Got current system");

    let relevant_checks: HashMap<String, Derivation> = checks_structure["checks"][&current_system]
        .as_object()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "checks flake output is not a map of derivations: {}",
                serde_json::to_string_pretty(&checks_structure[&current_system]).unwrap()
            )
        })?
        .into_iter()
        .map(|(k, v)| {
            Ok::<_, serde_json::Error>((k.to_string(), serde_json::from_value(v.clone())?))
        })
        .collect::<Result<_, _>>()?;

    info!(
        "Checking the following attributes: {}",
        relevant_checks
            .keys()
            .map(|k| k.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut check_infos: Vec<CheckTestCase> = vec![];

    for (check_name, derivation) in relevant_checks {
        let nix_check_string = format!(".#checks.{current_system}.{check_name}");
        let info =
            crate::nix::build(nix_check_string.clone(), nix::BuildMode::DryRun, max_jobs).await?;
        info!("Running {:?} -> {}", nix_check_string, info[0].drv_path);
        let start = Instant::now();
        let build_status = crate::nix::build(nix_check_string, nix::BuildMode::Real, max_jobs)
            .await
            .is_ok();
        let duration = start.elapsed();

        check_infos.push(CheckTestCase {
            name: derivation.name,
            result: {
                if build_status {
                    info!("{check_name} ran succesfully");
                    CheckResult::Success
                } else {
                    error!("{check_name} failed");
                    CheckResult::Failure {
                        log_output: {
                            match crate::nix::log(&info[0].drv_path).await {
                                Ok(out) => out,
                                Err(error) => {
                                    tracing::warn!(?error, "nix-log failed");
                                    format!("nix-log call failed: {error}")
                                }
                            }
                        },
                    }
                }
            },
            duration,
        })
    }

    let test_cases: Vec<TestCase> = check_infos
        .into_iter()
        .map(|c| match c.result {
            CheckResult::Success => {
                debug!(name = %c.name, "Creating success case");
                TestCaseBuilder::success(
                    &c.name,
                    junit_report::Duration::milliseconds(c.duration.as_millis() as i64),
                )
                .build()
            }
            CheckResult::Failure { log_output } => {
                debug!(name = %c.name, "Creating failure case");
                let mut tc = TestCaseBuilder::failure(
                    &c.name,
                    junit_report::Duration::milliseconds(c.duration.as_millis() as i64),
                    "nix check",
                    "build failed",
                )
                .build();

                tc.set_system_out(&log_output);
                tc
            }
        })
        .collect();

    let test_suite = TestSuiteBuilder::new("nix flake checks")
        .add_testcases(test_cases)
        .build();

    let report = ReportBuilder::new().add_testsuite(test_suite).build();

    let mut out: Vec<u8> = vec![];
    report.write_xml(&mut out).unwrap();

    tokio::fs::write(output_path, out)
        .await
        .with_context(|| format!("Could not open path at '{}'", output_path))?;

    Ok(())
}
