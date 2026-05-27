use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::Error;

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

    let mut cmd = Command::new("git");
    cmd.args(&args).current_dir(cwd);
    for (key, value) in envs {
        cmd.env(key, value);
    }

    let output = cmd.output().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            Error::GitNotFound
        } else {
            Error::GitCommand {
                command: command.clone(),
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
        Err(Error::GitCommand { command, stderr })
    }
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
}
