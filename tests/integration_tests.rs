//! Integration tests for nsmount using testcontainers
//!
//! These tests build and run nsmount inside a Linux container, enabling testing from macOS.
//!
//! Run with: cargo test --test integration_tests
//!
//! Requirements:
//! - Docker must be running
//!
//! First run will take longer (~2-5 minutes) to build the binary inside the container.
//! Subsequent runs use cached artifacts.

mod common;
mod nsmount;

use nsmount::{run_nsmount, run_nsmount_privileged};

// ============================================================================
// Error Cases
// ============================================================================

#[tokio::test]
async fn test_wrong_number_of_arguments() {
    let (_, _, exit) = run_nsmount(&[]).await;
    assert_ne!(exit, 0);
}

#[tokio::test]
async fn test_nonexistent_from_pid() {
    let (stdout, stderr, exit) = run_nsmount(&["99999", "/tmp", "1", "/tmp"]).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 1, "Output:\n{}", output);
    assert!(
        output.contains("No such file or directory"),
        "Expected 'No such file or directory' error. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_nonexistent_from_path() {
    let script = r#"
        unshare --mount sleep 60 &
        CHILD_PID=$!
        sleep 1

        /build/target/release/nsmount $CHILD_PID /nonexistent/path 1 /tmp
        EXIT_CODE=$?

        kill $CHILD_PID 2>/dev/null
        wait $CHILD_PID 2>/dev/null || true
        exit $EXIT_CODE
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 1, "Output:\n{}", output);
    assert!(
        output.contains("No such file or directory"),
        "Expected 'No such file or directory' error. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_nonexistent_to_path() {
    let script = r#"
        unshare --mount sleep 60 &
        CHILD_PID=$!
        sleep 1

        /build/target/release/nsmount 1 /tmp $CHILD_PID /nonexistent/path
        EXIT_CODE=$?

        kill $CHILD_PID 2>/dev/null
        wait $CHILD_PID 2>/dev/null || true
        exit $EXIT_CODE
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 1, "Output:\n{}", output);
    assert!(
        output.contains("No such file or directory"),
        "Expected 'No such file or directory' error. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_without_cap_sys_admin() {
    let (stdout, stderr, exit) = run_nsmount(&["1", "/tmp", "1", "/tmp"]).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 1, "Output:\n{}", output);
    assert!(
        output.contains("Permission denied") || output.contains("Operation not permitted"),
        "Expected permission error. Output:\n{}",
        output
    );
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[tokio::test]
async fn test_mount_directory_between_namespaces() {
    let script = r#"
        set -e

        mkdir -p /tmp/src /tmp/dst
        echo "hello from source" > /tmp/src/testfile.txt

        unshare --mount sleep 60 &
        CHILD_PID=$!
        sleep 1

        /build/target/release/nsmount $CHILD_PID /tmp/src 1 /tmp/dst

        CONTENT=$(cat /tmp/dst/testfile.txt)
        echo "MOUNT_CONTENT=$CONTENT"

        kill $CHILD_PID 2>/dev/null
        wait $CHILD_PID 2>/dev/null || true
        exit 0
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 0, "Output:\n{}", output);
    assert!(
        output.contains("MOUNT_CONTENT=hello from source"),
        "Expected mounted file content to be visible. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_mount_from_init_into_child() {
    let script = r#"
        set -e

        mkdir -p /tmp/src /tmp/dst
        echo "from init namespace" > /tmp/src/init_file.txt

        unshare --mount sleep 60 &
        CHILD_PID=$!
        sleep 1

        /build/target/release/nsmount 1 /tmp/src $CHILD_PID /tmp/dst

        CONTENT=$(nsenter --mount --target $CHILD_PID cat /tmp/dst/init_file.txt)
        echo "MOUNT_CONTENT=$CONTENT"

        kill $CHILD_PID 2>/dev/null
        wait $CHILD_PID 2>/dev/null || true
        exit 0
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 0, "Output:\n{}", output);
    assert!(
        output.contains("MOUNT_CONTENT=from init namespace"),
        "Expected init namespace content visible in child. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_mount_single_file() {
    let script = r#"
        set -e

        echo "single file content" > /tmp/src_file.txt
        touch /tmp/dst_file.txt

        unshare --mount sleep 60 &
        CHILD_PID=$!
        sleep 1

        /build/target/release/nsmount $CHILD_PID /tmp/src_file.txt 1 /tmp/dst_file.txt

        CONTENT=$(cat /tmp/dst_file.txt)
        echo "MOUNT_CONTENT=$CONTENT"

        kill $CHILD_PID 2>/dev/null
        wait $CHILD_PID 2>/dev/null || true
        exit 0
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 0, "Output:\n{}", output);
    assert!(
        output.contains("MOUNT_CONTENT=single file content"),
        "Expected single file mount to be visible. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_mount_same_pid() {
    let script = r#"
        set -e

        mkdir -p /tmp/src /tmp/dst
        echo "same pid content" > /tmp/src/samepid.txt

        unshare --mount sleep 60 &
        CHILD_PID=$!
        sleep 1

        /build/target/release/nsmount $CHILD_PID /tmp/src $CHILD_PID /tmp/dst

        CONTENT=$(nsenter --mount --target $CHILD_PID cat /tmp/dst/samepid.txt)
        echo "MOUNT_CONTENT=$CONTENT"

        kill $CHILD_PID 2>/dev/null
        wait $CHILD_PID 2>/dev/null || true
        exit 0
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 0, "Output:\n{}", output);
    assert!(
        output.contains("MOUNT_CONTENT=same pid content"),
        "Expected mount within same namespace to work. Output:\n{}",
        output
    );
}

#[tokio::test]
async fn test_mount_persists_after_source_exits() {
    let script = r#"
        set -e

        mkdir -p /tmp/src /tmp/dst
        echo "persistent content" > /tmp/src/persist.txt

        unshare --mount sleep 60 &
        SRC_PID=$!
        sleep 1

        /build/target/release/nsmount $SRC_PID /tmp/src 1 /tmp/dst

        kill $SRC_PID 2>/dev/null
        wait $SRC_PID 2>/dev/null || true

        CONTENT=$(cat /tmp/dst/persist.txt)
        echo "MOUNT_CONTENT=$CONTENT"
        exit 0
    "#;

    let (stdout, stderr, exit) = run_nsmount_privileged(script).await;
    let output = format!("{}{}", stdout, stderr);

    assert_eq!(exit, 0, "Output:\n{}", output);
    assert!(
        output.contains("MOUNT_CONTENT=persistent content"),
        "Expected mount to persist after source process exits. Output:\n{}",
        output
    );
}
