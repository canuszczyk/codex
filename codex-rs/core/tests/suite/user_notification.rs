#![cfg(not(target_os = "windows"))]

use std::collections::HashSet;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_protocol::user_input::UserInput;
use core_test_support::fs_wait;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;

use responses::ev_assistant_message;
use responses::ev_completed;
use responses::ev_function_call;
use responses::ev_response_created;
use responses::sse;
use responses::start_mock_server;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "flaky on ubuntu-24.04-arm - aarch64-unknown-linux-gnu"]
// The notify script gets far enough to create (and therefore surface) the file,
// but hasn’t flushed the JSON yet. Reading an empty file produces EOF while parsing
// a value at line 1 column 0. May be caused by a slow runner.
async fn summarize_context_three_requests_and_instructions() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let sse1 = sse(vec![ev_assistant_message("m1", "Done"), ev_completed("r1")]);

    responses::mount_sse_once(&server, sse1).await;

    let notify_dir = TempDir::new()?;
    let notify_script = write_notify_script(&notify_dir)?;

    let notify_start_file = notify_dir.path().join("notify-start.txt");
    let notify_complete_file = notify_dir.path().join("notify-complete.txt");
    let notify_script_str = notify_script.to_str().unwrap().to_string();

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |cfg| cfg.notify = Some(vec![notify_script_str]))
        .build(&server)
        .await?;

    // 1) Normal user input – should hit server once.
    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "hello world".into(),
            }],
        })
        .await?;
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    // We fork the notify script, so we need to wait for it to write to the file.
    fs_wait::wait_for_path_exists(&notify_start_file, Duration::from_secs(5)).await?;
    fs_wait::wait_for_path_exists(&notify_complete_file, Duration::from_secs(5)).await?;
    let start_payload_raw = tokio::fs::read_to_string(&notify_start_file).await?;
    let start_payload: Value = serde_json::from_str(&start_payload_raw)?;
    assert_eq!(start_payload["type"], json!("agent-turn-start"));
    assert_eq!(start_payload["input-messages"], json!(["hello world"]));

    let notify_payload_raw = tokio::fs::read_to_string(&notify_complete_file).await?;
    let payload: Value = serde_json::from_str(&notify_payload_raw)?;
    assert_eq!(payload["type"], json!("agent-turn-complete"));
    assert_eq!(payload["input-messages"], json!(["hello world"]));
    assert_eq!(payload["last-assistant-message"], json!("Done"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "flaky on ubuntu-24.04-arm - aarch64-unknown-linux-gnu"]
async fn interrupt_turn_emits_stop_notification() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "sleep 60".to_string(),
    ];
    let args = json!({
        "command": command,
        "timeout_ms": 60_000
    })
    .to_string();
    let sse_body = sse(vec![
        ev_function_call("call-stop", "shell", &args),
        ev_completed("resp-stop"),
    ]);

    responses::mount_sse_once(&server, sse_body).await;

    let notify_dir = TempDir::new()?;
    let notify_script = write_notify_script(&notify_dir)?;
    let notify_start_file = notify_dir.path().join("notify-start.txt");
    let notify_stop_file = notify_dir.path().join("notify-stop.txt");
    let notify_script_str = notify_script.to_str().unwrap().to_string();

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |cfg| cfg.notify = Some(vec![notify_script_str]))
        .build(&server)
        .await?;

    codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "start sleep".into(),
            }],
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;

    codex.submit(Op::Interrupt).await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;

    fs_wait::wait_for_path_exists(&notify_start_file, Duration::from_secs(5)).await?;
    fs_wait::wait_for_path_exists(&notify_stop_file, Duration::from_secs(5)).await?;

    let stop_payload_raw = tokio::fs::read_to_string(&notify_stop_file).await?;
    let stop_payload: Value = serde_json::from_str(&stop_payload_raw)?;
    assert_eq!(stop_payload["type"], json!("agent-turn-stop"));
    assert_eq!(stop_payload["input-messages"], json!(["start sleep"]));

    let start_payload_raw = tokio::fs::read_to_string(&notify_start_file).await?;
    let start_payload: Value = serde_json::from_str(&start_payload_raw)?;
    assert_eq!(start_payload["type"], json!("agent-turn-start"));
    assert_eq!(start_payload["input-messages"], json!(["start sleep"]));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "flaky on ubuntu-24.04-arm - aarch64-unknown-linux-gnu"]
async fn exec_approval_emits_prompt_and_restart_notifications() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let command = vec![
        "bash".to_string(),
        "-lc".to_string(),
        "echo hello".to_string(),
    ];
    let args = json!({
        "command": command,
        "timeout_ms": 1_000
    })
    .to_string();
    let sse_body = sse(vec![
        ev_response_created("resp-approval"),
        ev_function_call("call-approval", "shell", &args),
        ev_completed("resp-approval"),
    ]);

    responses::mount_sse_once(&server, sse_body).await;

    let notify_dir = TempDir::new()?;
    let notify_script = write_logged_notify_script(&notify_dir)?;
    let notify_script_str = notify_script.to_str().unwrap().to_string();

    let TestCodex { codex, .. } = test_codex()
        .with_config(move |cfg| {
            cfg.notify = Some(vec![notify_script_str]);
            cfg.approval_policy = AskForApproval::UnlessTrusted;
        })
        .build(&server)
        .await?;

    let submission_id = codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "run command".into(),
            }],
        })
        .await?;

    let exec_event =
        wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecApprovalRequest(_))).await;
    if !matches!(exec_event, EventMsg::ExecApprovalRequest(_)) {
        panic!("expected exec approval request, got {exec_event:?}");
    }

    let prompt_file =
        fs_wait::wait_for_matching_file(notify_dir.path(), Duration::from_secs(5), |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with("notify-user-prompt-"))
                .unwrap_or(false)
        })
        .await?;
    let prompt_payload_raw = tokio::fs::read_to_string(&prompt_file).await?;
    let prompt_payload: Value = serde_json::from_str(&prompt_payload_raw)?;
    assert_eq!(prompt_payload["type"], json!("agent-turn-user-prompt"));

    let existing_start_files = list_matching_files(notify_dir.path(), "notify-start");
    let existing_start_names: HashSet<String> = existing_start_files
        .iter()
        .filter_map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(std::string::ToString::to_string)
        })
        .collect();

    codex
        .submit(Op::ExecApproval {
            id: submission_id,
            decision: ReviewDecision::Approved,
        })
        .await?;

    let new_start_file =
        fs_wait::wait_for_matching_file(notify_dir.path(), Duration::from_secs(5), move |path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| {
                    name.starts_with("notify-start-") && !existing_start_names.contains(name)
                })
                .unwrap_or(false)
        })
        .await?;

    wait_for_event(&codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let start_payload_raw = tokio::fs::read_to_string(&new_start_file).await?;
    let start_payload: Value = serde_json::from_str(&start_payload_raw)?;
    assert_eq!(start_payload["type"], json!("agent-turn-start"));
    assert_eq!(start_payload["input-messages"], json!(["run command"]));

    Ok(())
}

fn write_notify_script(dir: &TempDir) -> anyhow::Result<PathBuf> {
    let script = dir.path().join("notify.sh");
    std::fs::write(
        &script,
        r#"#!/bin/bash
set -e
payload="${@: -1}"
notify_dir=$(dirname "${0}")
if [[ "${payload}" == *"agent-turn-start"* ]]; then
  echo -n "${payload}" > "${notify_dir}/notify-start.txt"
elif [[ "${payload}" == *"agent-turn-stop"* ]]; then
  echo -n "${payload}" > "${notify_dir}/notify-stop.txt"
elif [[ "${payload}" == *"agent-turn-user-prompt"* ]]; then
  echo -n "${payload}" > "${notify_dir}/notify-user-prompt.txt"
else
  echo -n "${payload}" > "${notify_dir}/notify-complete.txt"
fi"#,
    )?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    Ok(script)
}

fn write_logged_notify_script(dir: &TempDir) -> anyhow::Result<PathBuf> {
    let script = dir.path().join("notify-log.sh");
    std::fs::write(
        &script,
        r#"#!/bin/bash
set -e
payload="${@: -1}"
notify_dir=$(dirname "${0}")
if [[ "${payload}" == *"agent-turn-start"* ]]; then
  prefix="notify-start"
elif [[ "${payload}" == *"agent-turn-stop"* ]]; then
  prefix="notify-stop"
elif [[ "${payload}" == *"agent-turn-user-prompt"* ]]; then
  prefix="notify-user-prompt"
else
  prefix="notify-other"
fi
stamp=$(date +%s%N)
echo -n "${payload}" > "${notify_dir}/${prefix}-${stamp}.txt"
"#,
    )?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    Ok(script)
}

fn list_matching_files(dir: &Path, prefix: &str) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(std::result::Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.starts_with(prefix))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}
