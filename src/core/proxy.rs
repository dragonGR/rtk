//! High-performance CLI proxy execution module.
//!
//! Provides raw and smart-filtered execution of child processes, preserving
//! signal handling, stdout/stderr streaming, exit code propagation, and usage tracking.

use anyhow::{Context, Result};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;

use crate::core::tracking;
use crate::core::utils::{exit_code_from_status, resolved_command};
use crate::discover::lexer::shell_split;

/// Options for configuring proxy execution behavior.
#[derive(Debug, Clone, Default)]
pub struct ProxyOptions {
    /// Force raw un-filtered passthrough even if RTK has a filter for the command.
    pub raw: bool,
    /// Verbosity level.
    pub verbose: u8,
}

static PROXY_CHILD_PID: AtomicU32 = AtomicU32::new(0);

#[cfg(unix)]
#[allow(unsafe_code)]
unsafe extern "C" fn handle_signal(sig: libc::c_int) {
    let pid = PROXY_CHILD_PID.load(Ordering::SeqCst);
    if pid != 0 {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
        libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0);
    }
    libc::signal(sig, libc::SIG_DFL);
    libc::raise(sig);
}

fn setup_signal_handlers() {
    #[cfg(unix)]
    #[allow(unsafe_code)]
    {
        unsafe {
            libc::signal(
                libc::SIGINT,
                handle_signal as *const () as libc::sighandler_t,
            );
            libc::signal(
                libc::SIGTERM,
                handle_signal as *const () as libc::sighandler_t,
            );
        }
    }
}

struct ChildGuard(Option<std::process::Child>);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        PROXY_CHILD_PID.store(0, Ordering::SeqCst);
    }
}

/// Executes a command under the RTK proxy.
///
/// If `opts.raw` is false, attempts to auto-discover if RTK has a built-in filter
/// for the target command and auto-routes it. Otherwise, streams raw output while
/// tracking token consumption.
pub fn run_proxy(args: &[OsString], opts: &ProxyOptions) -> Result<i32> {
    let mut raw = opts.raw;
    let effective_args = if !args.is_empty() && (args[0] == "-r" || args[0] == "--raw") {
        raw = true;
        &args[1..]
    } else {
        args
    };

    if effective_args.is_empty() {
        anyhow::bail!(
            "proxy requires a command to execute\nUsage: rtk proxy [-r|--raw] <command> [args...]"
        );
    }

    let timer = tracking::TimedExecution::start();

    // If a single quoted arg contains spaces, split it respecting quotes.
    let (cmd_name, cmd_args): (String, Vec<String>) = if effective_args.len() == 1 {
        let full = effective_args[0].to_string_lossy();
        let parts = shell_split(&full);
        if parts.len() > 1 {
            (parts[0].clone(), parts[1..].to_vec())
        } else {
            (full.into_owned(), vec![])
        }
    } else {
        (
            effective_args[0].to_string_lossy().into_owned(),
            effective_args[1..]
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect(),
        )
    };

    let full_cmd_str = if cmd_args.is_empty() {
        cmd_name.clone()
    } else {
        format!("{} {}", cmd_name, cmd_args.join(" "))
    };

    // Smart auto-routing: check if RTK has a registered filter rewrite for this command
    if !raw {
        if let Some(rewritten) = crate::discover::registry::rewrite_command(&full_cmd_str, &[], &[]) {
            if rewritten.starts_with("rtk ") && !rewritten.starts_with("rtk proxy") {
                if opts.verbose > 0 {
                    eprintln!("Proxy auto-routing to: {}", rewritten);
                }
                let rewritten_args = shell_split(&rewritten);
                if rewritten_args.len() > 1 {
                    let sub_args: Vec<OsString> = rewritten_args[1..]
                        .iter()
                        .map(OsString::from)
                        .collect();
                    return run_rewritten_command(&sub_args, opts.verbose);
                }
            }
        }
    }

    if opts.verbose > 0 {
        eprintln!("Proxy mode (raw): {}", full_cmd_str);
    }

    setup_signal_handlers();

    let mut child = ChildGuard(Some(
        resolved_command(cmd_name.as_ref())
            .args(&cmd_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to execute command: {}", cmd_name))?,
    ));

    if let Some(ref inner) = child.0 {
        PROXY_CHILD_PID.store(inner.id(), Ordering::SeqCst);
    }

    let inner = child.0.as_mut().context("Child process missing")?;
    let stdout_pipe = inner
        .stdout
        .take()
        .context("Failed to capture child stdout")?;
    let stderr_pipe = inner
        .stderr
        .take()
        .context("Failed to capture child stderr")?;

    const CAP: usize = 1_048_576;

    let stdout_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stdout_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            if captured.len() < CAP {
                let take = count.min(CAP - captured.len());
                captured.extend_from_slice(&buf[..take]);
            }
            let mut out = std::io::stdout().lock();
            out.write_all(&buf[..count])?;
            out.flush()?;
        }

        Ok(captured)
    });

    let stderr_handle = thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut reader = stderr_pipe;
        let mut captured = Vec::new();
        let mut buf = [0u8; 8192];

        loop {
            let count = reader.read(&mut buf)?;
            if count == 0 {
                break;
            }
            if captured.len() < CAP {
                let take = count.min(CAP - captured.len());
                captured.extend_from_slice(&buf[..take]);
            }
            let mut err = std::io::stderr().lock();
            err.write_all(&buf[..count])?;
            err.flush()?;
        }

        Ok(captured)
    });

    let status = child
        .0
        .take()
        .context("Child process missing")?
        .wait()
        .context(format!("Failed waiting for command: {}", cmd_name))?;

    let stdout_bytes = stdout_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stdout streaming thread panicked"))??;
    let stderr_bytes = stderr_handle
        .join()
        .map_err(|_| anyhow::anyhow!("stderr streaming thread panicked"))??;

    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);
    let full_output = format!("{}{}", stdout, stderr);

    timer.track(
        &full_cmd_str,
        &format!("rtk proxy {}", full_cmd_str),
        &full_output,
        &full_output,
    );

    Ok(exit_code_from_status(&status, &cmd_name))
}

fn run_rewritten_command(args: &[OsString], verbose: u8) -> Result<i32> {
    let mut cmd_args = vec![OsString::from("rtk")];
    cmd_args.extend_from_slice(args);
    if verbose > 0 {
        cmd_args.push(OsString::from("-v"));
    }
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args(&cmd_args[1..])
        .spawn()?;
    let status = child.wait()?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_options_default() {
        let opts = ProxyOptions::default();
        assert!(!opts.raw);
        assert_eq!(opts.verbose, 0);
    }
}
