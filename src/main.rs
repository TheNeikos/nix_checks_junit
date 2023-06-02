use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod nix;

use clap::Parser;
use junit_report::{Duration, ReportBuilder, TestCase, TestCaseBuilder, TestSuiteBuilder};
use tracing::debug;
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
        output_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = tracing_subscriber::filter::EnvFilter::from_default_env();
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
        Command::RunChecks { output_path } => {
            run_checks(&output_path).await?;
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
}

async fn run_checks(output_path: &Path) -> anyhow::Result<()> {
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

    let mut check_infos: Vec<CheckTestCase> = vec![];

    for (check_name, derivation) in relevant_checks {
        let info = crate::nix::build(
            format!(".#checks.{current_system}.{check_name}"),
            nix::BuildMode::DryRun,
        )
        .await?;
        let build_status = crate::nix::build(
            format!(".#checks.{current_system}.{check_name}"),
            nix::BuildMode::Real,
        )
        .await
        .is_ok();

        check_infos.push(CheckTestCase {
            name: derivation.name,
            result: {
                if build_status {
                    CheckResult::Success
                } else {
                    CheckResult::Failure {
                        log_output: crate::nix::log(&info[0].drv_path).await?,
                    }
                }
            },
        })
    }

    let test_cases: Vec<TestCase> = check_infos
        .into_iter()
        .map(|c| match c.result {
            CheckResult::Success => {
                debug!(name = %c.name, "Creating success case");
                TestCaseBuilder::success(&c.name, Duration::ZERO).build()
            }
            CheckResult::Failure { log_output } => {
                debug!(name = %c.name, "Creating failure case");
                let mut tc =
                    TestCaseBuilder::failure(&c.name, Duration::ZERO, "nix check", "build failed")
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

    tokio::fs::write(output_path, out).await?;

    //  {
    //   "checks": {
    //     "x86_64-linux": {
    //       "nix_checks_junit": {
    //         "name": "nix_checks_junit-0.1.0",
    //         "type": "derivation"
    //       },
    //       "nix_checks_junit-clippy": {
    //         "name": "nix_checks_junit-clippy-0.1.0",
    //         "type": "derivation"
    //       },
    //       "nix_checks_junit-fmt": {
    //         "name": "nix_checks_junit-fmt-0.1.0",
    //         "type": "derivation"
    //       }
    //     }
    //   },
    // }

    Ok(())
}
