use serde::Serialize;
use std::path::PathBuf;
use tracing::error;
use tracing::warn;

#[derive(Debug, Default)]
pub(crate) struct UserNotifier {
    notify_command: Option<Vec<String>>,
}

impl UserNotifier {
    pub(crate) fn notify(&self, notification: &UserNotification) {
        if let Some(notify_command) = &self.notify_command
            && !notify_command.is_empty()
        {
            self.invoke_notify(notify_command, notification)
        }
    }

    fn invoke_notify(&self, notify_command: &[String], notification: &UserNotification) {
        let Ok(json) = serde_json::to_string(&notification) else {
            error!("failed to serialise notification payload");
            return;
        };

        let mut command = std::process::Command::new(&notify_command[0]);
        if notify_command.len() > 1 {
            command.args(&notify_command[1..]);
        }
        command.arg(json);

        // Fire-and-forget – we do not wait for completion.
        if let Err(e) = command.spawn() {
            warn!("failed to spawn notifier '{}': {e}", notify_command[0]);
        }
    }

    pub(crate) fn new(notify: Option<Vec<String>>) -> Self {
        Self {
            notify_command: notify,
        }
    }
}

/// User can configure a program that will receive notifications. Each
/// notification is serialized as JSON and passed as an argument to the
/// program.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[allow(clippy::enum_variant_names)]
pub(crate) enum UserNotification {
    #[serde(rename_all = "kebab-case")]
    AgentTurnStart {
        thread_id: String,
        turn_id: String,
        cwd: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,
    },
    #[serde(rename_all = "kebab-case")]
    AgentTurnUserPrompt {
        thread_id: String,
        turn_id: String,
        cwd: String,
        prompt: UserPromptNotification,
    },
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        thread_id: String,
        turn_id: String,
        cwd: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,

        /// The last message sent by the assistant in the turn.
        last_assistant_message: Option<String>,
    },
    #[serde(rename_all = "kebab-case")]
    AgentTurnStop {
        thread_id: String,
        turn_id: String,
        cwd: String,

        /// Messages that the user sent to the agent to initiate the turn.
        input_messages: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "prompt-type", rename_all = "kebab-case")]
pub(crate) enum UserPromptNotification {
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_user_notification() -> Result<()> {
        let start_notification = UserNotification::AgentTurnStart {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "12345".to_string(),
            cwd: "/Users/example/project".to_string(),
            input_messages: vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
        };
        let start_serialized = serde_json::to_string(&start_notification)?;
        assert_eq!(
            start_serialized,
            r#"{"type":"agent-turn-start","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"12345","cwd":"/Users/example/project","input-messages":["Rename `foo` to `bar` and update the callsites."]}"#
        );

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
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"12345","cwd":"/Users/example/project","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );

        let stop_notification = UserNotification::AgentTurnStop {
            thread_id: "b5f6c1c2-1111-2222-3333-444455556666".to_string(),
            turn_id: "54321".to_string(),
            cwd: "/Users/example/project".to_string(),
            input_messages: vec!["hello world".to_string()],
        };
        let stop_serialized = serde_json::to_string(&stop_notification)?;
        assert_eq!(
            stop_serialized,
            r#"{"type":"agent-turn-stop","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"54321","cwd":"/Users/example/project","input-messages":["hello world"]}"#
        );

        let prompt_notification = UserNotification::AgentTurnUserPrompt {
            thread_id: "thread-1".to_string(),
            turn_id: "turn-2".to_string(),
            cwd: "/tmp".to_string(),
            prompt: UserPromptNotification::ExecApproval {
                command: vec!["bash".to_string(), "-lc".to_string(), "ls".to_string()],
                reason: Some("safety".to_string()),
            },
        };
        let prompt_serialized = serde_json::to_string(&prompt_notification)?;
        assert_eq!(
            prompt_serialized,
            r#"{"type":"agent-turn-user-prompt","thread-id":"thread-1","turn-id":"turn-2","cwd":"/tmp","prompt":{"prompt-type":"exec-approval","command":["bash","-lc","ls"],"reason":"safety"}}"#
        );
        Ok(())
    }
}
