use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use serde::Serialize;

use crate::Hook;
use crate::HookEvent;
use crate::HookOutcome;
use crate::HookPayload;
use crate::command_from_argv;

/// Legacy notify payload appended as the final argv argument for backward compatibility.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[allow(clippy::enum_variant_names)]
enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnStart {
        thread_id: String,
        turn_id: String,
        cwd: String,
        input_messages: Vec<String>,
    },
    #[serde(rename_all = "kebab-case")]
    AgentTurnUserPrompt {
        thread_id: String,
        turn_id: String,
        cwd: String,
        prompt: LegacyUserPromptNotification,
    },
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        thread_id: String,
        turn_id: String,
        cwd: String,
        input_messages: Vec<String>,
        last_assistant_message: Option<String>,
    },
    #[serde(rename_all = "kebab-case")]
    AgentTurnStop {
        thread_id: String,
        turn_id: String,
        cwd: String,
        input_messages: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "prompt-type", rename_all = "kebab-case")]
enum LegacyUserPromptNotification {
    ExecApproval {
        command: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    ApplyPatchApproval {
        files: Vec<PathBuf>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

pub fn legacy_notify_json(hook_event: &HookEvent, cwd: &Path) -> Result<String, serde_json::Error> {
    match hook_event {
        HookEvent::BeforeAgent { event } => {
            serde_json::to_string(&UserNotification::AgentTurnStart {
                thread_id: event.thread_id.to_string(),
                turn_id: event.turn_id.clone(),
                cwd: cwd.display().to_string(),
                input_messages: event.input_messages.clone(),
            })
        }
        HookEvent::AfterAgent { event } => {
            serde_json::to_string(&UserNotification::AgentTurnComplete {
                thread_id: event.thread_id.to_string(),
                turn_id: event.turn_id.clone(),
                cwd: cwd.display().to_string(),
                input_messages: event.input_messages.clone(),
                last_assistant_message: event.last_assistant_message.clone(),
            })
        }
        HookEvent::AgentUserPrompt { event } => {
            let prompt = match &event.prompt {
                crate::types::UserPromptNotification::ExecApproval { command, reason } => {
                    LegacyUserPromptNotification::ExecApproval {
                        command: command.clone(),
                        reason: reason.clone(),
                    }
                }
                crate::types::UserPromptNotification::ApplyPatchApproval { files, reason } => {
                    LegacyUserPromptNotification::ApplyPatchApproval {
                        files: files.clone(),
                        reason: reason.clone(),
                    }
                }
            };
            serde_json::to_string(&UserNotification::AgentTurnUserPrompt {
                thread_id: event.thread_id.to_string(),
                turn_id: event.turn_id.clone(),
                cwd: cwd.display().to_string(),
                prompt,
            })
        }
        HookEvent::AgentStop { event } => {
            serde_json::to_string(&UserNotification::AgentTurnStop {
                thread_id: event.thread_id.to_string(),
                turn_id: event.turn_id.clone(),
                cwd: cwd.display().to_string(),
                input_messages: event.input_messages.clone(),
            })
        }
        _ => Err(serde_json::Error::io(std::io::Error::other(
            "legacy notify payload is not supported for this event type",
        ))),
    }
}

pub fn notify_hook(argv: Vec<String>) -> Hook {
    let argv = Arc::new(argv);
    Hook {
        func: Arc::new(move |payload: &HookPayload| {
            let argv = Arc::clone(&argv);
            Box::pin(async move {
                let mut command = match command_from_argv(&argv) {
                    Some(command) => command,
                    None => return HookOutcome::Continue,
                };
                if let Ok(notify_payload) = legacy_notify_json(&payload.hook_event, &payload.cwd) {
                    command.arg(notify_payload);
                }

                // Backwards-compat: match legacy notify behavior (argv + JSON arg, fire-and-forget).
                command
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());

                let _ = command.spawn();
                HookOutcome::Continue
            })
        }),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use codex_protocol::ThreadId;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use serde_json::json;

    use super::*;

    fn expected_notification_json() -> Value {
        json!({
            "type": "agent-turn-complete",
            "thread-id": "b5f6c1c2-1111-2222-3333-444455556666",
            "turn-id": "12345",
            "cwd": "/Users/example/project",
            "input-messages": ["Rename `foo` to `bar` and update the callsites."],
            "last-assistant-message": "Rename complete and verified `cargo build` succeeds.",
        })
    }

    #[test]
    fn test_user_notification() -> Result<()> {
        let notification = UserNotification::AgentTurnComplete {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "12345".to_string(),
            cwd: "/Users/example/project".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            last_assistant_message: Some(
                "Rename complete and verified `cargo build` succeeds.".to_string(),
            ),
        };
        let serialized = serde_json::to_string(&notification)?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual, expected_notification_json());
        Ok(())
    }

    #[test]
    fn legacy_notify_json_matches_historical_wire_shape() -> Result<()> {
        let hook_event = HookEvent::AfterAgent {
            event: crate::HookEventAfterAgent {
                thread_id: ThreadId::from_string("b5f6c1c2-1111-2222-3333-444455556666")
                    .expect("valid thread id"),
                turn_id: "12345".to_string(),
                input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
                last_assistant_message: Some(
                    "Rename complete and verified `cargo build` succeeds.".to_string(),
                ),
            },
        };

        let serialized = legacy_notify_json(&hook_event, Path::new("/Users/example/project"))?;
        let actual: Value = serde_json::from_str(&serialized)?;
        assert_eq!(actual, expected_notification_json());

        Ok(())
    }
}
