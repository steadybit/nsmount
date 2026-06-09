# nsmount

`nsmount` is a utility to mount a path from one process' mount namespace to another process' mount namespace.

## How It Works

1. Opens mount namespace file descriptors for both the source and target PIDs via `/proc/<pid>/ns/mnt`.
2. Enters the source namespace using `setns()` and clones the mount at `from-path` using the `open_tree` syscall.
3. Enters the target namespace using `setns()` and attaches the cloned mount at `to-path` using the `move_mount` syscall.

## Usage

```
nsmount <from-pid> <from-path> <to-pid> <to-path>

ARGS:
    <from-pid>     PID of the process to mount from
    <from-path>    Path to mount from
    <to-pid>       PID of the process to mount to
    <to-path>      Path to mount to
```

## Requirements

- Linux kernel **5.2+** (required for the `open_tree` and `move_mount` syscalls)
- `CAP_SYS_ADMIN` in both source and target mount namespaces (typically root)

## Building

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [cross](https://github.com/cross-rs/cross) for cross-compilation
- [Docker](https://www.docker.com/) (required by `cross` to run the build containers)
- [GNU Make](https://www.gnu.org/software/make/)

### Build

```bash
make build
```

Cross-compiles release binaries for `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`. The artefacts are placed under `target/<triple>/release/`.

### Run Tests

```bash
cargo test
```

Integration tests under `tests/` use [testcontainers](https://testcontainers.com/), so a working Docker installation is required.

## License

MIT - see [LICENSE](LICENSE).
