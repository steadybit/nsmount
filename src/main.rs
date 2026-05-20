#[cfg(not(target_os = "linux"))]
fn main() {
	eprintln!("nsmount is only supported on Linux");
	std::process::exit(1);
}

#[cfg(target_os = "linux")]
use std::{
	os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
	path::PathBuf,
};

#[cfg(target_os = "linux")]
use libc::{self, c_ulong};
#[cfg(target_os = "linux")]
use nix::{
	self,
	errno::Errno,
	fcntl,
	NixPath,
	Result,
	sched::{CloneFlags, setns}, sys::stat::Mode,
};
#[cfg(target_os = "linux")]
use structopt::StructOpt;

// gettid(3) only got a libc wrapper in glibc 2.30. The libc crate
// declares `extern "C" { fn gettid(); }`, and a reference to it is
// pulled in transitively by the Rust standard library, which would
// push nsmount's minimum required glibc to 2.30 and exclude RHEL 8
// / Debian 10 (both shipping glibc 2.28).
//
// We define a local, exported `gettid` that issues the raw syscall.
// The static linker resolves every reference to this definition, so
// no GLIBC_2.30 versioned import is emitted into the binary.
// scripts/check-glibc.sh guards against regressions in CI.
#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "C" fn gettid() -> libc::pid_t {
	unsafe { libc::syscall(libc::SYS_gettid) as libc::pid_t }
}

#[cfg(target_os = "linux")]
#[derive(StructOpt, Debug)]
#[structopt(
name = "nsmount",
about = "Mount a path from one process' mount namespace to another process' mount namespace"
)]
struct Opt {
	#[structopt(help = "PID of the process to mount from")]
	from_pid: u32,

	#[structopt(help = "Path to mount from")]
	from_path: PathBuf,

	#[structopt(help = "PID of the process to mount to")]
	to_pid: u32,

	#[structopt(help = "Path to mount to")]
	to_path: PathBuf,
}

#[cfg(target_os = "linux")]
const NO_FD: Option<&'static OwnedFd> = None;
#[cfg(target_os = "linux")]
const NO_PATH: Option<&'static [u8]> = None;

#[cfg(target_os = "linux")]
fn main() {
	let opts = Opt::from_args();

	let fd_from_mntns = open_ns(opts.from_pid, "mnt");
	let fd_to_mntns = open_ns(opts.to_pid, "mnt");

	if let Err(e) = setns(fd_from_mntns, CloneFlags::empty()) {
		fatal(format!("setns {}", opts.from_pid).as_str(), e);
	}

	let fd_from = match open_tree(
		NO_FD,
		Some(&opts.from_path),
		OpenTreeFlag::OPEN_TREE_CLONE | OpenTreeFlag::OPEN_TREE_CLOEXEC,
	) {
		Ok(fd) => fd,
		Err(e) => fatal(&opts.from_path.display().to_string(), e),
	};

	if let Err(e) = setns(fd_to_mntns, CloneFlags::empty()) {
		fatal(format!("setns {}", opts.to_pid).as_str(), e);
	}

	if let Err(e) = move_mount(
		Some(fd_from.as_fd()),
		NO_PATH,
		NO_FD,
		Some(&opts.to_path),
		MoveMountFlag::MOVE_MOUNT_F_EMPTY_PATH,
	) {
		fatal(&opts.to_path.display().to_string(), e);
	}
}

#[cfg(target_os = "linux")]
fn fatal(context: &str, err: Errno) -> ! {
	eprintln!("nsmount: {}: {}", context, err.desc());
	std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn open_ns(pid: u32, ns: &str) -> OwnedFd {
	let path = PathBuf::from("/proc")
		.join(pid.to_string())
		.join("ns")
		.join(ns.to_string());
	match fcntl::open(&path, fcntl::OFlag::O_RDONLY, Mode::empty()) {
		Ok(fd) => unsafe { OwnedFd::from_raw_fd(fd) },
		Err(e) => fatal(&path.display().to_string(), e),
	}
}

#[cfg(target_os = "linux")]
::bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct OpenTreeFlag: libc::c_uint {
        const OPEN_TREE_CLONE = libc::OPEN_TREE_CLONE;
        const OPEN_TREE_CLOEXEC = libc::OPEN_TREE_CLOEXEC;
    }
}

#[cfg(target_os = "linux")]
fn open_tree<Fd: AsFd, P1: ?Sized + NixPath>(
	dfd: Option<Fd>,
	pathname: Option<&P1>,
	flags: OpenTreeFlag,
) -> Result<OwnedFd> {
	let ret = with_opt_nix_path(pathname, |p| unsafe {
		libc::syscall(libc::SYS_open_tree, at_fd(dfd), p, flags.bits() as c_ulong) as libc::c_int
	})?;

	Errno::result(ret).map(|r| unsafe { OwnedFd::from_raw_fd(r) })
}

#[cfg(target_os = "linux")]
::bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(transparent)]
    pub struct MoveMountFlag: libc::c_uint {
        const MOVE_MOUNT_F_SYMLINKS =  0x00000001;
        const MOVE_MOUNT_F_AUTOMOUNTS =  0x00000002;
        const MOVE_MOUNT_F_EMPTY_PATH =  0x00000004;
        const MOVE_MOUNT_T_SYMLINKS =  0x00000010;
        const MOVE_MOUNT_T_AUTOMOUNTS =  0x00000020;
        const MOVE_MOUNT_T_EMPTY_PATH =  0x00000040;
    }
}

#[cfg(target_os = "linux")]
fn move_mount<Fd1: AsFd, Fd2: AsFd, P1: ?Sized + NixPath, P2: ?Sized + NixPath>(
	from_dfd: Option<Fd1>,
	from_path: Option<&P1>,
	to_dfd: Option<Fd2>,
	to_path: Option<&P2>,
	flags: MoveMountFlag,
) -> Result<()> {
	let ret = with_opt_nix_path(from_path, |f| {
		with_opt_nix_path(to_path, |t| unsafe {
			libc::syscall(
				libc::SYS_move_mount,
				at_fd(from_dfd),
				f,
				at_fd(to_dfd),
				t,
				flags.bits() as c_ulong,
			)
		})
	})??;

	Errno::result(ret).map(drop)
}

#[cfg(target_os = "linux")]
fn with_opt_nix_path<P, T, F>(p: Option<&P>, f: F) -> Result<T>
	where
		P: ?Sized + NixPath,
		F: FnOnce(*const libc::c_char) -> T,
{
	match p {
		Some(path) => path.with_nix_path(|p_str| f(p_str.as_ptr())),
		None => "".with_nix_path(|p_str| f(p_str.as_ptr())),
	}
}

#[cfg(target_os = "linux")]
fn at_fd<Fd: AsFd>(fd: Option<Fd>) -> libc::c_int {
	match fd {
		None => -libc::EBADF,
		Some(fd) => fd.as_fd().as_raw_fd(),
	}
}
