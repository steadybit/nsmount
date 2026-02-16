#[allow(deprecated)]
use bollard::container::{LogsOptions, WaitContainerOptions};
use bollard::Docker;
use futures::StreamExt;
use std::path::PathBuf;
use std::time::Duration;
use testcontainers::core::{AccessMode, Mount};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::OnceCell;

const RUST_IMAGE: &str = "rust";
const RUST_TAG: &str = "1-trixie";

static BUILD_COMPLETE: OnceCell<()> = OnceCell::const_new();

pub fn project_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir)
}

pub fn test_cache_dir() -> PathBuf {
    project_dir().join(".testcontainers")
}

fn ensure_cache_dirs() -> std::io::Result<()> {
    let cache = test_cache_dir();
    std::fs::create_dir_all(cache.join("registry"))?;
    std::fs::create_dir_all(cache.join("git"))?;
    std::fs::create_dir_all(cache.join("target"))?;
    Ok(())
}

// ============================================================================
// Build
// ============================================================================

pub async fn ensure_binary_built() {
    BUILD_COMPLETE
        .get_or_init(|| async {
            ensure_cache_dirs().expect("Failed to create cache directories");

            let project = project_dir();
            let cache = test_cache_dir();

            let container = GenericImage::new(RUST_IMAGE, RUST_TAG)
                .with_mount(
                    Mount::bind_mount(project.to_string_lossy(), "/app")
                        .with_access_mode(AccessMode::ReadOnly),
                )
                .with_mount(Mount::bind_mount(
                    cache.join("registry").to_string_lossy(),
                    "/usr/local/cargo/registry",
                ))
                .with_mount(Mount::bind_mount(
                    cache.join("git").to_string_lossy(),
                    "/usr/local/cargo/git",
                ))
                .with_mount(Mount::bind_mount(
                    cache.join("target").to_string_lossy(),
                    "/build/target",
                ))
                .with_env_var("CARGO_TARGET_DIR", "/build/target")
                .with_env_var("RUST_BACKTRACE", "1")
                .with_working_dir("/app")
                .with_cmd(["cargo", "build", "--release"])
                .with_startup_timeout(Duration::from_secs(300))
                .start()
                .await
                .expect("Failed to start build container");

            let (_, stderr, exit_code) = wait_and_get_output(&container).await;

            if exit_code != 0 {
                panic!("Build failed with exit code {}: {}", exit_code, stderr);
            }
        })
        .await;
}

// ============================================================================
// Command Execution
// ============================================================================

pub async fn run_in_container(cmd: &[&str], privileged: bool) -> (String, String, i64) {
    let cache = test_cache_dir();
    let cmd_strings: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();

    let mut image = GenericImage::new(RUST_IMAGE, RUST_TAG)
        .with_cmd(cmd_strings)
        .with_startup_timeout(Duration::from_secs(60))
        .with_mount(
            Mount::bind_mount(cache.join("target").to_string_lossy(), "/build/target")
                .with_access_mode(AccessMode::ReadOnly),
        );

    if privileged {
        image = image.with_host_config_modifier(|hc| {
            hc.privileged = Some(true);
        });
    }

    let container = image.start().await.expect("Failed to start container");

    wait_and_get_output(&container).await
}

async fn wait_and_get_output(container: &ContainerAsync<GenericImage>) -> (String, String, i64) {
    let docker = Docker::connect_with_local_defaults().expect("Failed to connect to Docker");
    let container_id = container.id();

    #[allow(deprecated)]
    let wait_options = WaitContainerOptions {
        condition: "not-running",
    };

    let mut wait_stream = docker.wait_container(container_id, Some(wait_options));
    let exit_code = match wait_stream.next().await {
        Some(Ok(response)) => response.status_code,
        Some(Err(e)) => {
            eprintln!("Error waiting for container: {}", e);
            1
        }
        None => 1,
    };

    #[allow(deprecated)]
    let log_options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        ..Default::default()
    };

    let mut logs_stream = docker.logs(container_id, Some(log_options));
    let mut stdout = String::new();
    let mut stderr = String::new();

    while let Some(result) = logs_stream.next().await {
        match result {
            Ok(output) => match output {
                bollard::container::LogOutput::StdOut { message } => {
                    stdout.push_str(&String::from_utf8_lossy(&message));
                }
                bollard::container::LogOutput::StdErr { message } => {
                    stderr.push_str(&String::from_utf8_lossy(&message));
                }
                _ => {}
            },
            Err(e) => {
                eprintln!("Error reading logs: {}", e);
                break;
            }
        }
    }

    (stdout, stderr, exit_code)
}
