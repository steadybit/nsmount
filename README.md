# nsmount

`nsmount` is a utility to mount a path from one process' mount namespace to another process' mount namespace.

It works by:

1. Opening mount namespace file descriptors for both the source and target PIDs via `/proc/<pid>/ns/mnt`
2. Entering the source namespace using `setns()` and cloning the mount at `from-path` using the `open_tree` syscall
3. Entering the target namespace using `setns()` and attaching the cloned mount at `to-path` using the `move_mount` syscall

Requires Linux 5.2+ due to the use of `open_tree` and `move_mount` syscalls.

## Usage

```
	nsmount <from-pid> <from-path> <to-pid> <to-path>
```
