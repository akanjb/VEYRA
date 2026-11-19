//! The documented usage fields of each wire format, and their canonical form.
//!
//! - OpenAI: `prompt_tokens`/`input_tokens` include cached tokens, which are reported
//!   in `*_tokens_details.cached_tokens`; reasoning is part of output. Streamed Chat
//!   Completions carry usage only when the client sets `stream_options.include_usage`.
//! - Anthropic: the Messages `usage` block, read with the same rules as Claude Code's
//!   logs. In a stream, `message_start` carries the input side and `message_delta` the
//!   running output; later values replace earlier ones.
//! - Gemini: `usageMetadata`, where `promptTokenCount` includes cached tokens and
//!   thoughts are reported apart from candidates.

use serde_json::{Map, Value};
use veyra_domain::accuracy::{Accuracy, AccuracyMap};
use veyra_domain::capture::Exchange;
use veyra_domain::event::{Granularity, ServiceTier};
use veyra_domain::identity::Interface;
use veyra_domain::money::NanoUsd;
use veyra_domain::source::RawUsage;
use veyra_domain::tokens::TokenCounts;

use super::{Route, Wire};
use crate::claude_code::api_usage;

/// What a response has said about itself so far.
#[derive(Debug, Default)]
pub struct Seen {
    id: Option<String>,
    model: Option<String>,
    service_tier: Option<String>,
    usage: Option<Map<String, Value>>,
}

fn text(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

impl Seen {
    /// Keeps the identifiers, model and usage of one response object or stream event.
    pub fn absorb(&mut self, wire: Wire, value: &Value) {
        match wire {
            Wire::OpenAi => {
                // Responses API stream events wrap the response; other events carry none.
                let streamed = value
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| t.starts_with("response."));
                let body = if streamed {
                    match value.get("response") {
                        Some(response) => response,
                        None => return,
                    }
                } else {
                    value
                };
                self.identify(text(body, "id"), text(body, "model"));
                if let Some(tier) = text(body, "service_tier") {
                    self.service_tier = Some(tier);
                }
                if let Some(Value::Object(usage)) = body.get("usage") {
                    self.usage = Some(usage.clone());
                }
            }
            Wire::Anthropic => {
                let message = value.get("message").unwrap_or(value);
                self.identify(text(message, "id"), text(message, "model"));
                for usage in [message.get("usage"), value.get("usage")] {
                    if let Some(Value::Object(usage)) = usage {
                        self.merge_usage(usage);
                    }
                }
            }
            Wire::Gemini => {
                self.identify(text(value, "responseId"), text(value, "modelVersion"));
                if let Some(Value::Object(usage)) = value.get("usageMetadata") {
                    self.usage = Some(usage.clone());
                }
            }
        }
    }

    fn identify(&mut self, id: Option<String>, model: Option<String>) {
        if id.is_some() {
            self.id = id;
        }
        if model.is_some() {
            self.model = model;
        }
    }

    fn merge_usage(&mut self, usage: &Map<String, Value>) {
        let kept = self.usage.get_or_insert_with(Map::new);
        for (key, value) in usage {
            if !value.is_null() {
                kept.insert(key.clone(), value.clone());
            }
        }
    }
}

fn count(usage: &Map<String, Value>, path: &[&str]) -> Option<u64> {
    let (last, parents) = path.split_last()?;
    let mut object = usage;
    for parent in parents {
        object = object.get(*parent)?.as_object()?;
    }
    object.get(*last)?.as_u64()
}

struct Reading {
    tokens: TokenCounts,
    tier: ServiceTier,
    reasoning_consistent: bool,
    cost: Option<NanoUsd>,
}

fn openai(seen: &Seen, usage: &Map<String, Value>, gateway: bool) -> Reading {
    let input = count(usage, &["prompt_tokens"]).or_else(|| count(usage, &["input_tokens"]));
    let output = count(usage, &["completion_tokens"]).or_else(|| count(usage, &["output_tokens"]));
    let cached = count(usage, &["prompt_tokens_details", "cached_tokens"])
        .or_else(|| count(usage, &["input_tokens_details", "cached_tokens"]));
    let thinking = count(usage, &["completion_tokens_details", "reasoning_tokens"])
        .or_else(|| count(usage, &["output_tokens_details", "reasoning_tokens"]));
    let cached = cached.zip(input).map(|(c, i)| c.min(i));
    let reasoning = match (thinking, output) {
        (Some(t), Some(o)) if t <= o => Some(t),
        _ => None,
    };
    let tier = match seen.service_tier.as_deref() {
        Some("flex") => ServiceTier::Flex,
        Some("priority") => ServiceTier::Priority,
        _ => ServiceTier::Standard,
    };
    Reading {
        tokens: TokenCounts {
            input_uncached: input.map(|i| i - cached.unwrap_or(0)),
            cache_read: input.map(|_| cached.unwrap_or(0)),
            output_total: output,
            reasoning,
            ..TokenCounts::default()
        },
        tier,
        reasoning_consistent: thinking.is_none() || reasoning.is_some(),
        // Only routing services report what they charged.
        cost: if gateway {
            usage.get("cost").and_then(Value::as_f64).and_then(nano_usd)
        } else {
            None
        },
    }
}

fn gemini(usage: &Map<String, Value>) -> Reading {
    let input = count(usage, &["promptTokenCount"]).unwrap_or(0);
    let cached = count(usage, &["cachedContentTokenCount"])
        .unwrap_or(0)
        .min(input);
    let tool = count(usage, &["toolUsePromptTokenCount"]).unwrap_or(0);
    let candidates = count(usage, &["candidatesTokenCount"]).unwrap_or(0);
    let thoughts = count(usage, &["thoughtsTokenCount"]).unwrap_or(0);
    Reading {
        tokens: TokenCounts {
            input_uncached: Some(input - cached + tool),
            cache_read: Some(cached),
            output_total: Some(candidates + thoughts),
            reasoning: Some(thoughts),
            tool_tokens: Some(tool),
            ..TokenCounts::default()
        },
        tier: ServiceTier::Standard,
        reasoning_consistent: true,
        cost: None,
    }
}

/// US dollars as nano-dollars; `None` for negative, non-finite or absurd amounts.
fn nano_usd(usd: f64) -> Option<NanoUsd> {
    let nanos = (usd * 1e9).round();
    if !(0.0..1e15).contains(&nanos) {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "range-checked against one million dollars just above"
    )]
    Some(NanoUsd(nanos as i64))
}

/// The Gemini model named in a request path such as `/v1beta/models/gemini-2.5-pro:generateContent`.
fn model_in_path(path: &str) -> Option<String> {
    let (_, rest) = path.split_once("/models/")?;
    let (model, _) = rest.split_once(':')?;
    (!model.is_empty()).then(|| model.to_owned())
}

/// The usage record of a finished response. A response that reported no usage is
/// still one request when it named itself; its tokens are then marked Partial.
pub fn raw_usage(route: &Route, seen: &Seen, exchange: &Exchange<'_>) -> Option<RawUsage> {
    let reading = match (&seen.usage, route.wire) {
        (Some(usage), Wire::OpenAi) => Some(openai(seen, usage, route.gateway.is_some())),
        (Some(usage), Wire::Anthropic) => {
            api_usage(&Value::Object(usage.clone())).map(|(tokens, tier, consistent)| Reading {
                tokens,
                tier,
                reasoning_consistent: consistent,
                cost: None,
            })
        }
        (Some(usage), Wire::Gemini) => Some(gemini(usage)),
        (None, _) => None,
    };
    if reading.is_none() && seen.id.is_none() {
        return None;
    }
    // Anthropic's request id is the key Claude Code's logs use for the same request.
    let id = match route.wire {
        Wire::Anthropic => exchange
            .request_id
            .map(str::to_owned)
            .or_else(|| seen.id.clone()),
        Wire::OpenAi | Wire::Gemini => seen.id.clone(),
    };
    let natural_key = match id {
        Some(id) => format!("{}:{id}", route.prefix),
        None => format!("{}:proxy-{}", route.prefix, exchange.fallback_id),
    };
    let model = seen.model.clone().or_else(|| match route.wire {
        Wire::Gemini => model_in_path(exchange.path),
        Wire::OpenAi | Wire::Anthropic => None,
    });
    let measured = reading.is_some();
    let reading = reading.unwrap_or(Reading {
        tokens: TokenCounts::default(),
        tier: ServiceTier::Standard,
        reasoning_consistent: true,
        cost: None,
    });
    let total = reading.tokens.total().unwrap_or(0);
    Some(RawUsage {
        natural_key,
        // Below a completed log line for the same request, which also knows the project.
        precedence: i64::try_from(total).unwrap_or(i64::MAX).min((1 << 52) - 1),
        occurred_at: exchange.at,
        model,
        provider: Some(route.provider.to_owned()),
        gateway: route.gateway.map(str::to_owned),
        tokens: reading.tokens,
        requests: Some(1),
        reported_cost: reading.cost,
        interface: Interface::Api,
        service_tier: reading.tier,
        inference_geo_us: false,
        granularity: Granularity::Request,
        session_key: None,
        parent_session_key: None,
        is_subagent: false,
        cwd: None,
        tool_version: None,
        accuracy: AccuracyMap {
            tokens: if measured {
                Accuracy::Exact
            } else {
                Accuracy::Partial
            },
            cache: if measured {
                Accuracy::Exact
            } else {
                Accuracy::Partial
            },
            reasoning: if reading.reasoning_consistent && measured {
                Accuracy::Exact
            } else {
                Accuracy::Partial
            },
            requests: Accuracy::Exact,
            cost: if reading.cost.is_some() {
                Accuracy::ProviderReported
            } else {
                Accuracy::Estimated
            },
        },
        tool_calls: Vec::new(),
    })
}
