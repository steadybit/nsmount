use super::common::{ensure_binary_built, run_in_container};

const BINARY_PATH: &str = "/build/target/release/nsmount";

pub async fn run_nsmount(args: &[&str]) -> (String, String, i64) {
    ensure_binary_built().await;

    let mut cmd = vec![BINARY_PATH];
    cmd.extend(args);

    run_in_container(&cmd, false).await
}

pub async fn run_nsmount_privileged(script: &str) -> (String, String, i64) {
    ensure_binary_built().await;

    run_in_container(&["bash", "-c", script], true).await
}
