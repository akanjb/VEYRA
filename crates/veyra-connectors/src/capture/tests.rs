#![allow(clippy::unwrap_used, reason = "test code: a failure aborts the test")]

use veyra_domain::accuracy::Accuracy;
use veyra_domain::event::ServiceTier;
use veyra_domain::money::NanoUsd;
use veyra_domain::source::RawUsage;
use veyra_domain::time::UtcMillis;
use veyra_domain::tokens::TokenCounts;

use veyra_domain::capture::{CaptureRoute as _, Exchange};

use super::{Route, routes};

fn route(prefix: &str) -> Route {
    routes().into_iter().find(|r| r.prefix == prefix).unwrap()
}

const AT: UtcMillis = UtcMillis(1_762_600_000_000);

/// Scans `body`, fed in chunks of `chunk` bytes.
fn scan(
    prefix: &str,
    content_type: &str,
    path: &str,
    request_id: Option<&str>,
    body: &str,
    chunk: usize,
) -> Option<RawUsage> {
    let route = route(prefix);
    let mut scanner = route.scanner(Some(content_type));
    for piece in body.as_bytes().chunks(chunk) {
        scanner.feed(piece);
    }
    scanner.finish(&Exchange {
        path,
        request_id,
        at: AT,
        fallback_id: "f1",
    })
}

const JSON: &str = "application/json";
const SSE: &str = "text/event-stream; charset=utf-8";

#[test]
fn openai_chat_completion_with_cache_and_reasoning() {
    let body = r#"{"id":"chatcmpl-1","object":"chat.completion","model":"gpt-5.2-2026-01-01",
        "service_tier":"flex","choices":[{"message":{"content":"secret text"}}],
        "usage":{"prompt_tokens":1200,"completion_tokens":300,"total_tokens":1500,
        "prompt_tokens_details":{"cached_tokens":1000},
        "completion_tokens_details":{"reasoning_tokens":200}}}"#;
    let raw = scan("openai", JSON, "/v1/chat/completions", None, body, 4096).unwrap();
    assert_eq!(raw.natural_key, "openai:chatcmpl-1");
    assert_eq!(raw.model.as_deref(), Some("gpt-5.2-2026-01-01"));
    assert_eq!(raw.provider.as_deref(), Some("openai"));
    assert_eq!(
        raw.tokens,
        TokenCounts {
            input_uncached: Some(200),
            cache_read: Some(1000),
            output_total: Some(300),
            reasoning: Some(200),
            ..TokenCounts::default()
        }
    );
    assert_eq!(raw.service_tier, ServiceTier::Flex);
    assert_eq!(raw.requests, Some(1));
    assert_eq!(raw.accuracy.tokens, Accuracy::Exact);
    assert_eq!(raw.reported_cost, None);
}

#[test]
fn openai_stream_reads_usage_from_the_last_chunk_even_split_across_reads() {
    let body = concat!(
        "data: {\"id\":\"chatcmpl-2\",\"model\":\"gpt-5.2\",\"choices\":[{\"delta\":{\"content\":\"Hi\"}}],\"usage\":null}\n\n",
        "data: {\"id\":\"chatcmpl-2\",\"model\":\"gpt-5.2\",\"choices\":[],",
        "\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"prompt_tokens_details\":{\"cached_tokens\":0}}}\n\n",
        "data: [DONE]\n\n"
    );
    for chunk in [1, 7, 4096] {
        let raw = scan("openai", SSE, "/v1/chat/completions", None, body, chunk).unwrap();
        assert_eq!(raw.natural_key, "openai:chatcmpl-2");
        assert_eq!(raw.tokens.input_uncached, Some(10));
        assert_eq!(raw.tokens.output_total, Some(5));
    }
}

#[test]
fn a_stream_without_usage_is_one_request_with_partial_tokens() {
    let body =
        "data: {\"id\":\"chatcmpl-3\",\"model\":\"gpt-5.2\",\"choices\":[]}\n\ndata: [DONE]\n\n";
    let raw = scan("openai", SSE, "/v1/chat/completions", None, body, 4096).unwrap();
    assert_eq!(raw.requests, Some(1));
    assert_eq!(raw.tokens, TokenCounts::default());
    assert_eq!(raw.accuracy.tokens, Accuracy::Partial);
}

#[test]
fn openai_responses_stream_uses_the_completed_event() {
    let body = concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-5.2\",\"usage\":null}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"text\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_1\",\"model\":\"gpt-5.2\",",
        "\"usage\":{\"input_tokens\":50,\"input_tokens_details\":{\"cached_tokens\":20},",
        "\"output_tokens\":30,\"output_tokens_details\":{\"reasoning_tokens\":10}}}}\n\n"
    );
    let raw = scan("openai", SSE, "/v1/responses", None, body, 13).unwrap();
    assert_eq!(raw.natural_key, "openai:resp_1");
    assert_eq!(
        raw.tokens,
        TokenCounts {
            input_uncached: Some(30),
            cache_read: Some(20),
            output_total: Some(30),
            reasoning: Some(10),
            ..TokenCounts::default()
        }
    );
}

#[test]
fn anthropic_stream_merges_start_and_delta_and_keys_by_request_id() {
    let body = concat!(
        "event: message_start\n",
        "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-5\",",
        "\"usage\":{\"input_tokens\":12,\"cache_read_input_tokens\":3000,\"cache_creation_input_tokens\":500,",
        "\"cache_creation\":{\"ephemeral_5m_input_tokens\":500,\"ephemeral_1h_input_tokens\":0},",
        "\"output_tokens\":1,\"service_tier\":\"standard\"}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"private\"}}\n\n",
        "event: message_delta\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":250}}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n"
    );
    let raw = scan(
        "anthropic",
        SSE,
        "/v1/messages",
        Some("req_011ABC"),
        body,
        9,
    )
    .unwrap();
    assert_eq!(
        raw.natural_key, "anthropic:req_011ABC",
        "the key Claude Code's logs use"
    );
    assert_eq!(raw.model.as_deref(), Some("claude-sonnet-4-5"));
    assert_eq!(
        raw.tokens,
        TokenCounts {
            input_uncached: Some(12),
            cache_read: Some(3000),
            cache_write_short: Some(500),
            cache_write_long: Some(0),
            output_total: Some(250),
            ..TokenCounts::default()
        }
    );
    assert!(raw.precedence < 1 << 52, "below a completed log line");
}

#[test]
fn gemini_sse_and_json_array_streams() {
    let usage = r#""usageMetadata":{"promptTokenCount":100,"cachedContentTokenCount":40,
        "candidatesTokenCount":20,"thoughtsTokenCount":15,"toolUsePromptTokenCount":5}"#;
    let expected = TokenCounts {
        input_uncached: Some(65),
        cache_read: Some(40),
        output_total: Some(35),
        reasoning: Some(15),
        tool_tokens: Some(5),
        ..TokenCounts::default()
    };
    let sse = format!(
        "data: {{\"responseId\":\"r1\",\"candidates\":[]}}\n\ndata: {{\"responseId\":\"r1\",\"modelVersion\":\"gemini-2.5-pro\",{}}}\n\n",
        usage.replace('\n', "")
    );
    let raw = scan(
        "gemini",
        SSE,
        "/v1beta/models/gemini-2.5-pro:streamGenerateContent",
        None,
        &sse,
        5,
    )
    .unwrap();
    assert_eq!(raw.natural_key, "gemini:r1");
    assert_eq!(raw.tokens, expected);

    let array = format!("[{{\"responseId\":\"r2\"}},{{\"responseId\":\"r2\",{usage}}}]");
    let raw = scan(
        "gemini",
        JSON,
        "/v1beta/models/gemini-2.5-flash:streamGenerateContent",
        None,
        &array,
        4096,
    )
    .unwrap();
    assert_eq!(
        raw.model.as_deref(),
        Some("gemini-2.5-flash"),
        "from the path"
    );
    assert_eq!(raw.tokens, expected);
}

#[test]
fn openrouter_reports_what_it_charged() {
    let body = r#"{"id":"gen-1","model":"anthropic/claude-sonnet-4.5",
        "usage":{"prompt_tokens":100,"completion_tokens":10,"cost":0.00123}}"#;
    let raw = scan("openrouter", JSON, "/v1/chat/completions", None, body, 4096).unwrap();
    assert_eq!(raw.natural_key, "openrouter:gen-1");
    assert_eq!(raw.gateway.as_deref(), Some("openrouter"));
    assert_eq!(raw.reported_cost, Some(NanoUsd(1_230_000)));
    assert_eq!(raw.accuracy.cost, Accuracy::ProviderReported);
}

#[test]
fn errors_and_unreadable_bodies_record_nothing() {
    let error = r#"{"error":{"type":"invalid_request_error","message":"bad"}}"#;
    assert!(scan("openai", JSON, "/v1/chat/completions", None, error, 4096).is_none());
    assert!(
        scan(
            "anthropic",
            JSON,
            "/v1/messages",
            Some("req_1"),
            "not json",
            4096
        )
        .is_none()
    );
}

#[test]
fn only_generation_requests_are_recorded() {
    let openai = route("openai");
    assert!(openai.records("POST", "/v1/chat/completions"));
    assert!(openai.records("POST", "/v1/responses"));
    assert!(openai.records("POST", "/v1/embeddings"));
    assert!(!openai.records("GET", "/v1/models"));
    assert!(!openai.records("POST", "/v1/files"));
    let anthropic = route("anthropic");
    assert!(anthropic.records("POST", "/v1/messages"));
    assert!(!anthropic.records("POST", "/v1/messages/count_tokens"));
    let gemini = route("gemini");
    assert!(gemini.records("POST", "/v1beta/models/gemini-2.5-pro:generateContent"));
    assert!(!gemini.records("POST", "/v1beta/models/gemini-2.5-pro:countTokens"));
}

#[test]
fn every_route_has_its_own_application() {
    let routes = routes();
    for route in &routes {
        assert_eq!(
            route.descriptor.id.as_str(),
            format!("{}-api", route.prefix)
        );
        assert!(route.upstream.starts_with("https://"));
    }
}
