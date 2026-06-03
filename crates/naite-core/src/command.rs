use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::Error;

const INDEX_LOCK_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
    Duration::from_millis(1_600),
];

pub(crate) fn run_git<I, S>(cwd: &Path, args: I) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run_git_allowing_exit_codes(cwd, args, &[])
}

pub(crate) fn run_git_allowing_exit_codes<I, S>(
    cwd: &Path,
    args: I,
    allowed_exit_codes: &[i32],
) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    run_git_command(cwd, args, allowed_exit_codes, &[])
}

pub(crate) fn run_provider_cli<I, S>(program: &str, cwd: &Path, args: I) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let command = format_command(program, &args);

    let output = Command::new(program)
        .args(&args)
        .current_dir(cwd)
        .output()
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                Error::ProviderCliNotFound {
                    program: program.to_string(),
                }
            } else {
                Error::ProviderCommand {
                    command: command.clone(),
                    stderr: source.to_string(),
                }
            }
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr = if stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr
        };
        Err(Error::ProviderCommand { command, stderr })
    }
}

/// Maximum number of characters of script output preserved in a validation
/// failure error. Chatty scripts keep only the tail, which is where build and
/// test tools print their failure summary.
const VALIDATION_OUTPUT_TAIL_CHARS: usize = 2000;

pub(crate) fn run_validation_script(
    script: &str,
    cwd: &Path,
    envs: &[(&str, &str)],
) -> Result<String, Error> {
    let command = script.to_string();
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(script).current_dir(cwd);
    for (key, value) in envs {
        cmd.env(key, value);
    }

    let output = cmd.output().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            Error::ProviderCliNotFound {
                program: "sh".to_string(),
            }
        } else {
            Error::ProviderCommand {
                command: command.clone(),
                stderr: source.to_string(),
            }
        }
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr = if stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr
        };
        Err(Error::ProviderCommand {
            command,
            stderr: output_tail(&stderr),
        })
    }
}

fn output_tail(output: &str) -> String {
    let total = output.chars().count();
    if total <= VALIDATION_OUTPUT_TAIL_CHARS {
        return output.to_string();
    }
    let tail: String = output
        .chars()
        .skip(total - VALIDATION_OUTPUT_TAIL_CHARS)
        .collect();
    format!("…{tail}")
}

pub(crate) fn run_git_with_env<I, S, K, V>(
    cwd: &Path,
    args: I,
    envs: &[(K, V)],
) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    run_git_with_env_allowing_exit_codes(cwd, args, envs, &[])
}

pub(crate) fn run_git_with_env_allowing_exit_codes<I, S, K, V>(
    cwd: &Path,
    args: I,
    envs: &[(K, V)],
    allowed_exit_codes: &[i32],
) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    K: AsRef<OsStr>,
    V: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let envs: Vec<(OsString, OsString)> = envs
        .iter()
        .map(|(key, value)| (key.as_ref().to_os_string(), value.as_ref().to_os_string()))
        .collect();
    run_git_command(cwd, args, allowed_exit_codes, &envs)
}

fn run_git_command(
    cwd: &Path,
    args: Vec<OsString>,
    allowed_exit_codes: &[i32],
    envs: &[(OsString, OsString)],
) -> Result<String, Error> {
    let command = format_git_command(&args);

    let mut next_delay = INDEX_LOCK_RETRY_DELAYS.iter();
    loop {
        match run_git_command_once(cwd, &args, allowed_exit_codes, envs, &command) {
            Err(Error::GitCommand { stderr, .. }) if is_index_lock_stderr(&stderr) => {
                let Some(delay) = next_delay.next() else {
                    return Err(Error::GitCommand { command, stderr });
                };
                thread::sleep(*delay);
            }
            result => return result,
        }
    }
}

fn run_git_command_once(
    cwd: &Path,
    args: &[OsString],
    allowed_exit_codes: &[i32],
    envs: &[(OsString, OsString)],
    command: &str,
) -> Result<String, Error> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    for (key, value) in envs {
        cmd.env(key, value);
    }

    let output = cmd.output().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            Error::GitNotFound
        } else {
            Error::GitCommand {
                command: command.to_string(),
                stderr: source.to_string(),
            }
        }
    })?;

    if output.status.success()
        || output
            .status
            .code()
            .is_some_and(|code| allowed_exit_codes.contains(&code))
    {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr = if stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr
        };
        Err(Error::GitCommand {
            command: command.to_string(),
            stderr,
        })
    }
}

fn is_index_lock_stderr(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains(".git/index.lock")
        || (lower.contains("unable to create") && lower.contains("index.lock"))
        || lower.contains("another git process seems to be running")
}

pub(crate) fn run_git_with_stdin<I, S>(cwd: &Path, args: I, stdin: &str) -> Result<String, Error>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();
    let command = format_git_command(&args);

    let mut child = Command::new("git")
        .args(&args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                Error::GitNotFound
            } else {
                Error::GitCommand {
                    command: command.clone(),
                    stderr: source.to_string(),
                }
            }
        })?;

    if let Some(mut child_stdin) = child.stdin.take() {
        child_stdin
            .write_all(stdin.as_bytes())
            .map_err(|source| Error::GitCommand {
                command: command.clone(),
                stderr: source.to_string(),
            })?;
    }

    let output = child
        .wait_with_output()
        .map_err(|source| Error::GitCommand {
            command: command.clone(),
            stderr: source.to_string(),
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stderr = if stderr.is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr
        };
        Err(Error::GitCommand { command, stderr })
    }
}

fn format_git_command(args: &[OsString]) -> String {
    format_command("git", args)
}

fn format_command(program: &str, args: &[OsString]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program.to_string());
    parts.extend(args.iter().map(|arg| arg.to_string_lossy().into_owned()));
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TempRepo;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn formats_git_command_for_errors() {
        let command = format_git_command(&[
            OsString::from("checkout"),
            OsString::from("--force"),
            OsString::from("feature/demo"),
        ]);

        assert_eq!(command, "git checkout --force feature/demo");
    }

    #[test]
    fn formats_provider_command_for_errors() {
        let command = format_command("gh", &[OsString::from("pr"), OsString::from("list")]);

        assert_eq!(command, "gh pr list");
    }

    #[test]
    fn validation_script_returns_stdout_on_success() {
        let dir = TempRepo::new("command-validation-success");

        let output = run_validation_script("echo hello", &dir.path, &[]).unwrap();

        assert_eq!(output.trim(), "hello");
    }

    #[test]
    fn validation_script_returns_stderr_tail_on_failure() {
        let dir = TempRepo::new("command-validation-failure");

        let result = run_validation_script("echo boom 1>&2; exit 3", &dir.path, &[]);

        match result {
            Err(Error::ProviderCommand { command, stderr }) => {
                assert_eq!(command, "echo boom 1>&2; exit 3");
                assert!(stderr.contains("boom"), "{stderr}");
            }
            other => panic!("expected provider command error, got {other:?}"),
        }
    }

    #[test]
    fn validation_script_exposes_env_vars_and_cwd() {
        let dir = TempRepo::new("command-validation-env");
        dir.write("marker.txt", "present\n");

        let result = run_validation_script(
            "test \"$NAITE_TARGET_BRANCH\" = main && test -f marker.txt",
            &dir.path,
            &[("NAITE_TARGET_BRANCH", "main")],
        );

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn output_tail_keeps_short_output_and_truncates_long_output() {
        assert_eq!(output_tail("short"), "short");

        let long = "x".repeat(VALIDATION_OUTPUT_TAIL_CHARS + 10);
        let tail = output_tail(&long);
        assert!(tail.starts_with('…'));
        assert_eq!(tail.chars().count(), VALIDATION_OUTPUT_TAIL_CHARS + 1);
    }

    #[test]
    fn retries_git_command_until_index_lock_clears() {
        let repo = TempRepo::init_with_commit("command-index-lock-retry");
        repo.write("file.txt", "changed\n");
        let lock_path = repo.path.join(".git/index.lock");
        fs::write(&lock_path, "locked").unwrap();

        let lock_for_thread = lock_path.clone();
        let remover = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            fs::remove_file(lock_for_thread).unwrap();
        });

        let result = run_git(&repo.path, ["add", "-A"]);
        remover.join().unwrap();

        assert!(result.is_ok());
        assert!(!lock_path.exists());
    }

    #[test]
    fn returns_index_lock_error_after_retry_budget_is_exhausted() {
        let repo = TempRepo::init_with_commit("command-index-lock-stale");
        repo.write("file.txt", "changed\n");
        let lock_path = repo.path.join(".git/index.lock");
        fs::write(&lock_path, "locked").unwrap();

        let result = run_git(&repo.path, ["add", "-A"]);

        assert!(lock_path.exists());
        match result {
            Err(Error::GitCommand { command, stderr }) => {
                assert_eq!(command, "git add -A");
                assert!(is_index_lock_stderr(&stderr), "{stderr}");
            }
            other => panic!("expected git index lock error, got {other:?}"),
        }
    }
}
