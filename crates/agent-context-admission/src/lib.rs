use thiserror::Error;

const MAX_SEGMENTS: usize = 8;
const MAX_SEGMENT_BYTES: usize = 8 * 1024;
const MAX_TOTAL_BYTES: usize = 24 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextAdmissionPolicy {
    pub max_segments: usize,
    pub max_segment_bytes: usize,
    pub max_total_bytes: usize,
    pub allowed_source_prefixes: Vec<String>,
}

impl Default for ContextAdmissionPolicy {
    fn default() -> Self {
        Self {
            max_segments: MAX_SEGMENTS,
            max_segment_bytes: MAX_SEGMENT_BYTES,
            max_total_bytes: MAX_TOTAL_BYTES,
            allowed_source_prefixes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSegment {
    pub source: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedContextSegment {
    pub source: String,
    pub content: String,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ContextAdmissionError {
    #[error("context admission policy is invalid")]
    InvalidPolicy,
    #[error("context exceeds segment budget")]
    SegmentBudget,
    #[error("context exceeds byte budget")]
    ByteBudget,
    #[error("context segment is invalid")]
    InvalidSegment,
    #[error("context contains prompt injection")]
    PromptInjection,
}

/// Admits bounded external data without changing its untrusted classification.
///
/// # Errors
///
/// Rejects malformed sources, excessive data and high-confidence instruction injection.
pub fn admit_untrusted_context(
    segments: Vec<ContextSegment>,
) -> Result<Vec<AdmittedContextSegment>, ContextAdmissionError> {
    admit_untrusted_context_with_policy(segments, &ContextAdmissionPolicy::default())
}

/// Admits external data under a caller policy bounded by global hard limits.
///
/// # Errors
///
/// Rejects unsafe policy values, disallowed sources, excessive data and prompt injection.
pub fn admit_untrusted_context_with_policy(
    segments: Vec<ContextSegment>,
    policy: &ContextAdmissionPolicy,
) -> Result<Vec<AdmittedContextSegment>, ContextAdmissionError> {
    validate_policy(policy)?;
    if segments.is_empty() || segments.len() > policy.max_segments {
        return Err(ContextAdmissionError::SegmentBudget);
    }
    let mut total_bytes = 0_usize;
    let mut admitted = Vec::with_capacity(segments.len());
    for segment in segments {
        let source = segment.source.trim();
        let content = segment.content.trim();
        if !valid_source(source)
            || !source_allowed(source, policy)
            || content.is_empty()
            || content.len() > policy.max_segment_bytes
        {
            return Err(ContextAdmissionError::InvalidSegment);
        }
        total_bytes = total_bytes
            .checked_add(content.len())
            .ok_or(ContextAdmissionError::ByteBudget)?;
        if total_bytes > policy.max_total_bytes {
            return Err(ContextAdmissionError::ByteBudget);
        }
        if high_confidence_prompt_injection(content) {
            return Err(ContextAdmissionError::PromptInjection);
        }
        admitted.push(AdmittedContextSegment {
            source: source.to_owned(),
            content: content.to_owned(),
        });
    }
    Ok(admitted)
}

fn validate_policy(policy: &ContextAdmissionPolicy) -> Result<(), ContextAdmissionError> {
    if policy.max_segments == 0
        || policy.max_segments > MAX_SEGMENTS
        || policy.max_segment_bytes == 0
        || policy.max_segment_bytes > MAX_SEGMENT_BYTES
        || policy.max_total_bytes == 0
        || policy.max_total_bytes > MAX_TOTAL_BYTES
        || policy.max_total_bytes < policy.max_segment_bytes
        || policy.allowed_source_prefixes.iter().any(|prefix| {
            prefix.is_empty() || prefix.len() > 128 || !valid_source(prefix.trim_end_matches(':'))
        })
    {
        return Err(ContextAdmissionError::InvalidPolicy);
    }
    Ok(())
}

fn source_allowed(source: &str, policy: &ContextAdmissionPolicy) -> bool {
    policy.allowed_source_prefixes.is_empty()
        || policy
            .allowed_source_prefixes
            .iter()
            .any(|prefix| source.starts_with(prefix))
}

fn valid_source(source: &str) -> bool {
    !source.is_empty()
        && source.len() <= 128
        && source
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b':' | b'_' | b'-'))
}

fn high_confidence_prompt_injection(content: &str) -> bool {
    let normalized = normalize_for_detection(content);
    [
        "ignore previous instructions",
        "ignore all previous instructions",
        "disregard previous instructions",
        "override system prompt",
        "reveal the system prompt",
        "you are now the system",
        "bypass tool approval",
        "disable safety policy",
        "忽略之前的指令",
        "忽略所有之前的指令",
        "覆盖系统提示词",
        "泄露系统提示词",
        "绕过工具审批",
        "关闭安全策略",
    ]
    .iter()
    .any(|pattern| normalized.contains(pattern))
}

fn normalize_for_detection(content: &str) -> String {
    content
        .chars()
        .filter_map(|character| match character {
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}' => None,
            '\u{ff01}'..='\u{ff5e}' => char::from_u32(u32::from(character) - 0xfee0),
            character if character.is_control() && !character.is_whitespace() => None,
            character => Some(character),
        })
        .collect::<String>()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment(content: &str) -> ContextSegment {
        ContextSegment {
            source: "connector:mail.message".to_owned(),
            content: content.to_owned(),
        }
    }

    #[test]
    fn admits_bounded_external_data() {
        let admitted = admit_untrusted_context(vec![segment("Meeting moved to 15:00")])
            .expect("admitted context");
        assert_eq!(admitted[0].content, "Meeting moved to 15:00");
    }

    #[test]
    fn rejects_plain_fullwidth_and_zero_width_injection() {
        for content in [
            "Ignore previous instructions",
            "Ｉｇｎｏｒｅ ｐｒｅｖｉｏｕｓ ｉｎｓｔｒｕｃｔｉｏｎｓ",
            "Ig\u{200b}nore previous instructions",
            "忽略\u{200b}之前的指令",
        ] {
            assert_eq!(
                admit_untrusted_context(vec![segment(content)]),
                Err(ContextAdmissionError::PromptInjection)
            );
        }
    }

    #[test]
    fn caller_policy_cannot_exceed_hard_limits_or_spoof_sources() {
        let oversized = ContextAdmissionPolicy {
            max_segments: MAX_SEGMENTS + 1,
            ..ContextAdmissionPolicy::default()
        };
        assert_eq!(
            admit_untrusted_context_with_policy(vec![segment("safe")], &oversized),
            Err(ContextAdmissionError::InvalidPolicy)
        );
        let connector_only = ContextAdmissionPolicy {
            allowed_source_prefixes: vec!["connector:mail.".to_owned()],
            ..ContextAdmissionPolicy::default()
        };
        assert_eq!(
            admit_untrusted_context_with_policy(
                vec![ContextSegment {
                    source: "event:mail.message".to_owned(),
                    content: "safe".to_owned(),
                }],
                &connector_only,
            ),
            Err(ContextAdmissionError::InvalidSegment)
        );
    }
}
