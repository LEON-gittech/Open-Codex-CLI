//! Shared formatting for user-facing resume command hints.

use codex_protocol::ThreadId;
use codex_shell_command::parse_command::shlex_join;

const DEFAULT_RESUME_COMMAND_NAME: &str = "codex";
const RESUME_COMMAND_NAME_ENV_VAR: &str = "CODEX_RESUME_COMMAND_NAME";

pub fn resume_command(thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    resume_command_for_program(&resume_command_name(), thread_name, thread_id)
}

pub fn resume_hint(_thread_name: Option<&str>, thread_id: Option<ThreadId>) -> Option<String> {
    resume_hint_for_program(&resume_command_name(), /*thread_name*/ None, thread_id)
}

fn resume_hint_for_program(
    program: &str,
    _thread_name: Option<&str>,
    thread_id: Option<ThreadId>,
) -> Option<String> {
    let thread_id = thread_id?;
    resume_command_for_program(program, /*thread_name*/ None, Some(thread_id))
}

fn resume_command_name() -> String {
    std::env::var(RESUME_COMMAND_NAME_ENV_VAR)
        .ok()
        .and_then(|name| normalize_resume_command_name(&name))
        .unwrap_or_else(|| DEFAULT_RESUME_COMMAND_NAME.to_string())
}

fn normalize_resume_command_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn resume_command_for_program(
    program: &str,
    thread_name: Option<&str>,
    thread_id: Option<ThreadId>,
) -> Option<String> {
    let resume_target = thread_name
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| thread_id.map(|thread_id| thread_id.to_string()));
    resume_target.map(|target| {
        let mut command = vec![program.to_string(), "resume".to_string()];
        if target.starts_with('-') {
            command.push("--".to_string());
        }
        command.push(target);
        shlex_join(&command)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn resume_hint_uses_id_directly_even_when_name_exists() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(
            resume_hint_for_program(
                DEFAULT_RESUME_COMMAND_NAME,
                Some("my-thread"),
                Some(thread_id)
            ),
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn resume_command_prefers_name_over_id() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command_for_program(
            DEFAULT_RESUME_COMMAND_NAME,
            Some("my-thread"),
            Some(thread_id),
        );
        assert_eq!(command, Some("codex resume my-thread".to_string()));
    }

    #[test]
    fn can_format_with_launcher_command_name() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command_for_program(
            "open-codex-dev",
            /*thread_name*/ None,
            Some(thread_id),
        );
        assert_eq!(
            command,
            Some("open-codex-dev resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn normalizes_launcher_command_name() {
        assert_eq!(
            normalize_resume_command_name(" open-codex-dev "),
            Some("open-codex-dev".to_string())
        );
        assert_eq!(normalize_resume_command_name("   "), None);
    }

    #[test]
    fn formats_thread_id_when_name_is_missing() {
        let thread_id = ThreadId::from_string("123e4567-e89b-12d3-a456-426614174000").unwrap();
        let command = resume_command_for_program(
            DEFAULT_RESUME_COMMAND_NAME,
            /*thread_name*/ None,
            Some(thread_id),
        );
        assert_eq!(
            command,
            Some("codex resume 123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn returns_none_without_a_resume_target() {
        let command = resume_command_for_program(
            DEFAULT_RESUME_COMMAND_NAME,
            /*thread_name*/ None,
            /*thread_id*/ None,
        );
        assert_eq!(command, None);
    }

    #[test]
    fn quotes_thread_names_when_needed() {
        let command = resume_command_for_program(
            DEFAULT_RESUME_COMMAND_NAME,
            Some("-starts-with-dash"),
            /*thread_id*/ None,
        );
        assert_eq!(
            command,
            Some("codex resume -- -starts-with-dash".to_string())
        );

        let command = resume_command_for_program(
            DEFAULT_RESUME_COMMAND_NAME,
            Some("two words"),
            /*thread_id*/ None,
        );
        assert_eq!(command, Some("codex resume 'two words'".to_string()));

        let command = resume_command_for_program(
            DEFAULT_RESUME_COMMAND_NAME,
            Some("quote'case"),
            /*thread_id*/ None,
        );
        assert_eq!(command, Some("codex resume \"quote'case\"".to_string()));
    }
}
