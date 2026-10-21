use crate::config::{Environment, environment};
use anyhow::{Result, bail, ensure};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn credential(name: &str) -> bool {
    ["KEY", "TOKEN", "SECRET", "PASSWORD"]
        .iter()
        .any(|s| name.to_uppercase().contains(s))
}
pub fn clean_environment() -> Environment {
    clean_environment_from(environment())
}
pub fn clean_environment_from(env: Environment) -> Environment {
    // Keep credentials and injected shell/rustc/git wrapper settings out of
    // child environments. Filesystem isolation remains the operator's concern.
    env.into_iter()
        .filter(|(k, _)| {
            [
                "PATH",
                "HOME",
                "TMPDIR",
                "TMP",
                "TEMP",
                "CARGO_HOME",
                "RUSTUP_HOME",
                "RUSTUP_TOOLCHAIN",
                "LANG",
                "LC_ALL",
                "SYSTEMROOT",
            ]
            .contains(&k.as_str())
        })
        .collect()
}
pub fn redact(mut text: String, env: &Environment) -> String {
    let mut secrets: Vec<_> = env
        .iter()
        .filter(|(k, v)| credential(k) && !v.is_empty())
        .map(|(_, v)| v)
        .collect();
    secrets.sort_by_key(|v| std::cmp::Reverse(v.len()));
    for secret in secrets {
        text = text.replace(secret, "[redacted]");
    }
    text
}
pub fn head(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
pub fn tail(s: &str, max: usize) -> String {
    let count = s.chars().count();
    s.chars().skip(count.saturating_sub(max)).collect()
}
pub struct Output {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
impl Output {
    pub fn text(&self) -> String {
        format!(
            "{}{}",
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }
}
fn capture(mut stream: impl Read) -> Vec<u8> {
    // Drain the pipe even after the cap to avoid child deadlocks.
    let mut output = Vec::new();
    let mut buf = [0; 8192];
    while let Ok(n) = stream.read(&mut buf) {
        if n == 0 {
            break;
        }
        let keep = n.min(8_000_000usize.saturating_sub(output.len()));
        output.extend_from_slice(&buf[..keep]);
    }
    output
}
pub fn execute(
    program: &str,
    args: &[&str],
    cwd: &Path,
    env: &Environment,
    timeout: Duration,
    input: &[u8],
) -> Result<Output> {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let stdin = child.stdin.take().unwrap();
    let input = input.to_vec();
    let writer = thread::spawn(move || {
        let mut stdin = stdin;
        let _ = stdin.write_all(&input);
    });
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || capture(stdout));
    let err = thread::spawn(move || capture(stderr));
    let start = Instant::now();
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status.code().unwrap_or(1);
        }
        if start.elapsed() >= timeout {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            break 124;
        }
        thread::sleep(Duration::from_millis(20));
    };
    // A command can exit before a background descendant closes its output pipes.
    #[cfg(unix)]
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = writer.join();
    Ok(Output {
        code,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}
pub fn run(program: &str, args: &[&str], cwd: &Path, trusted: bool) -> Result<String> {
    let env = if trusted {
        environment()
    } else {
        clean_environment()
    };
    let result = execute(program, args, cwd, &env, Duration::from_secs(600), &[])?;
    if result.code != 0 {
        bail!(
            "{program} exited {}: {}",
            result.code,
            tail(&redact(result.text(), &environment()), 3000)
        );
    }
    ensure!(
        result.stdout.len() < 8_000_000,
        "Command output exceeded limit"
    );
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}
