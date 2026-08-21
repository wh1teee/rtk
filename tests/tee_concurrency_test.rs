#[cfg(unix)]
use std::collections::HashSet;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::{Command, Stdio};

#[cfg(unix)]
fn tee_path(output: &[u8]) -> PathBuf {
    let text = String::from_utf8_lossy(output);
    let raw = text
        .lines()
        .find_map(|line| {
            line.strip_prefix("[full output: ")
                .and_then(|value| value.strip_suffix(']'))
        })
        .unwrap_or_else(|| panic!("missing tee hint in output: {text}"));
    let raw = raw.trim_matches('"');
    if let Some(relative) = raw.strip_prefix("~/") {
        return dirs::home_dir().expect("home directory").join(relative);
    }
    PathBuf::from(raw)
}

#[cfg(unix)]
#[test]
fn concurrent_test_commands_keep_tee_output_owned_by_caller() {
    const CALLERS: usize = 8;
    let temp = tempfile::tempdir().expect("tempdir");
    let tee_dir = temp.path().join("tee");
    let config_dir = temp.path().join("config");
    let mut children = Vec::with_capacity(CALLERS);

    for caller in 0..CALLERS {
        let marker = format!("CALLER_{caller}_UNIQUE");
        let script = format!(
            "i=0; while [ \"$i\" -lt 100 ]; do printf '{}_%s\\n' \"$i\"; i=$((i+1)); done; exit 17",
            marker
        );
        let child = Command::new(env!("CARGO_BIN_EXE_rtk"))
            .args(["test", "sh", "-c", &script])
            .env("RTK_TEE", "1")
            .env("RTK_TEE_DIR", &tee_dir)
            .env("XDG_CONFIG_HOME", &config_dir)
            .env(
                "RTK_DB_PATH",
                temp.path().join(format!("test-history-{caller}.db")),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn concurrent rtk test");
        children.push((marker, child));
    }

    let mut results = Vec::with_capacity(CALLERS);
    for (marker, child) in children {
        let output = child
            .wait_with_output()
            .expect("wait for concurrent rtk test");
        assert_eq!(output.status.code(), Some(17), "caller exit code changed");
        let mut combined = output.stdout;
        combined.extend_from_slice(&output.stderr);
        results.push((marker, tee_path(&combined)));
    }

    let unique_paths: HashSet<_> = results.iter().map(|(_, path)| path).collect();
    assert_eq!(
        unique_paths.len(),
        CALLERS,
        "tee path was shared across callers"
    );

    for (marker, path) in &results {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read caller tee log {}: {error}", path.display()));
        assert!(content.contains(marker), "caller lost its own tee output");
        for (other, _) in &results {
            if other != marker {
                assert!(
                    !content.contains(other),
                    "caller tee log contains another caller's output"
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn concurrent_err_commands_keep_tee_output_owned_by_caller() {
    const CALLERS: usize = 8;
    let temp = tempfile::tempdir().expect("tempdir");
    let tee_dir = temp.path().join("tee");
    let config_dir = temp.path().join("config");
    let mut children = Vec::with_capacity(CALLERS);

    for caller in 0..CALLERS {
        let marker = format!("ERR_CALLER_{caller}_UNIQUE");
        let script = format!(
            "i=0; while [ \"$i\" -lt 100 ]; do printf '{}_%s\\n' \"$i\"; i=$((i+1)); done; exit 23",
            marker
        );
        let child = Command::new(env!("CARGO_BIN_EXE_rtk"))
            .args(["err", &script])
            .env("RTK_TEE", "1")
            .env("RTK_TEE_DIR", &tee_dir)
            .env("XDG_CONFIG_HOME", &config_dir)
            .env(
                "RTK_DB_PATH",
                temp.path().join(format!("err-history-{caller}.db")),
            )
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn concurrent rtk err");
        children.push((marker, child));
    }

    let mut results = Vec::with_capacity(CALLERS);
    for (marker, child) in children {
        let output = child
            .wait_with_output()
            .expect("wait for concurrent rtk err");
        assert_eq!(output.status.code(), Some(23), "caller exit code changed");
        let mut combined = output.stdout;
        combined.extend_from_slice(&output.stderr);
        results.push((marker, tee_path(&combined)));
    }

    let unique_paths: HashSet<_> = results.iter().map(|(_, path)| path).collect();
    assert_eq!(
        unique_paths.len(),
        CALLERS,
        "tee path was shared across callers"
    );

    for (marker, path) in &results {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("read caller tee log {}: {error}", path.display()));
        assert!(content.contains(marker), "caller lost its own tee output");
        for (other, _) in &results {
            if other != marker {
                assert!(
                    !content.contains(other),
                    "caller tee log contains another caller's output"
                );
            }
        }
    }
}
