//! Subprocess helpers: bounded-time capture (`run_subprocess_with_timeout` in
//! `validators.py`) and POSIX process-group control for cancellable jobs.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

use wait_timeout::ChildExt;

use crate::errors::BigTubeError;
use crate::Result;

/// Run a command capturing stdout/stderr, killing it past `timeout`.
/// Returns `(exit_code, stdout, stderr)`. Output is drained on reader threads to
/// avoid pipe-buffer deadlock while waiting.
pub fn run_with_timeout(
    program: &str,
    args: &[String],
    env: &HashMap<String, String>,
    timeout: Duration,
) -> Result<(i32, String, String)> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(env)
        .spawn()?;

    let mut out = child.stdout.take().expect("piped stdout");
    let mut err = child.stderr.take().expect("piped stderr");
    let out_handle = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let err_handle = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });

    match child.wait_timeout(timeout)? {
        Some(status) => {
            let code = status.code().unwrap_or(-1);
            let stdout = out_handle.join().unwrap_or_default();
            let stderr = err_handle.join().unwrap_or_default();
            Ok((code, stdout, stderr))
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = out_handle.join();
            let _ = err_handle.join();
            Err(BigTubeError::Timeout(timeout))
        }
    }
}

/// Put a child in its own process group so the whole tree can be signalled.
/// Mirrors `start_new_session=True`. No-op on non-Unix.
#[cfg(unix)]
pub fn new_process_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(not(unix))]
pub fn new_process_group(_cmd: &mut Command) {}

/// Send SIGTERM (then SIGKILL after `grace`) to a child's process group,
/// mirroring `_terminate_process`. `pid` is the child's PID (== group id).
#[cfg(unix)]
pub fn terminate_group(pid: u32, grace: Duration) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;
    let pgid = Pid::from_raw(pid as i32);
    if killpg(pgid, Signal::SIGTERM).is_err() {
        return;
    }
    std::thread::sleep(grace);
    let _ = killpg(pgid, Signal::SIGKILL);
}

#[cfg(not(unix))]
pub fn terminate_group(_pid: u32, _grace: Duration) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> HashMap<String, String> {
        std::env::vars().collect()
    }

    #[test]
    fn captures_stdout_and_exit_code() {
        let (code, out, _err) = run_with_timeout(
            "sh",
            &["-c".into(), "printf hello".into()],
            &env(),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(code, 0);
        assert_eq!(out, "hello");
    }

    #[test]
    fn nonzero_exit_code() {
        let (code, _o, _e) = run_with_timeout(
            "sh",
            &["-c".into(), "exit 3".into()],
            &env(),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(code, 3);
    }

    #[test]
    fn times_out_long_command() {
        let r = run_with_timeout(
            "sh",
            &["-c".into(), "sleep 5".into()],
            &env(),
            Duration::from_millis(150),
        );
        assert!(matches!(r, Err(BigTubeError::Timeout(_))));
    }
}
