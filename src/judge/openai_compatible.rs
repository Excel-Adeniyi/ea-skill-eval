use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::{MetricScore, Trace};
use crate::judge::prompt::{build_user_message, SYSTEM_PROMPT};
use crate::judge::response::parse_judge_response;
use crate::judge::Judge;

/// Ollama's OpenAI-compatible endpoint, served locally by default.
pub const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";

/// A judge that speaks the OpenAI chat-completions protocol.
///
/// That protocol is the lingua franca: local Ollama models, Zhipu's GLM
/// endpoint and OpenAI itself all accept the same request shape. Swapping
/// between them is configuration, not code — which is why this one struct
/// covers every judge except Anthropic's.
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleJudge {
    client: reqwest::Client,
    base_url: String,
    model: String,
    api_key: Option<String>,
}

impl OpenAiCompatibleJudge {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            // Local 20-30B models take 40-70s per trace on consumer hardware.
            // The default timeout is far too short for that.
            .timeout(Duration::from_secs(300))
            .build()
            .context("could not build the HTTP client")?;

        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            api_key: None,
        })
    }

    /// A judge backed by a model already pulled into local Ollama.
    pub fn ollama(model: impl Into<String>) -> Result<Self> {
        Self::new(OLLAMA_BASE_URL, model)
    }

    /// Attach a bearer token, for hosted endpoints that need one.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl Judge for OpenAiCompatibleJudge {
    fn name(&self) -> String {
        format!("llm:{}", self.model)
    }

    async fn score(&self, trace: &Trace) -> Result<Vec<MetricScore>> {
        let request = ChatRequest {
            model: &self.model,
            stream: false,
            // Asking for a JSON object up front is what took the probe runs to
            // 4/4 parseable. `parse_judge_response` still defends against
            // fences and prose, because not every endpoint honours this.
            response_format: ResponseFormat { kind: "json_object" },
            messages: vec![
                ChatMessage {
                    role: "system",
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user",
                    content: build_user_message(trace),
                },
            ],
        };

        let url = format!("{}/chat/completions", self.base_url);
        let mut builder = self.client.post(&url).json(&request);

        if let Some(key) = &self.api_key {
            builder = builder.bearer_auth(key);
        }

        let response = builder
            .send()
            .await
            .with_context(|| format!("could not reach the judge at {url}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("could not read the judge's response body")?;

        if !status.is_success() {
            return Err(anyhow!("judge returned HTTP {status}: {body}"));
        }

        let parsed: ChatResponse = serde_json::from_str(&body)
            .with_context(|| format!("unexpected response envelope from {url}: {body}"))?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .ok_or_else(|| anyhow!("judge returned no choices"))?;

        parse_judge_response(&content)
            .with_context(|| format!("could not read scores from {}", self.model))
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    stream: bool,
    response_format: ResponseFormat,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_points_at_the_local_endpoint() {
        let judge = OpenAiCompatibleJudge::ollama("qwen3.6:latest").expect("should build");

        assert_eq!(judge.base_url, OLLAMA_BASE_URL);
        assert_eq!(judge.model(), "qwen3.6:latest");
        assert!(judge.api_key.is_none());
    }

    #[test]
    fn trailing_slashes_do_not_produce_a_double_slash_url() {
        let judge =
            OpenAiCompatibleJudge::new("http://example.test/v1/", "m").expect("should build");

        assert_eq!(judge.base_url, "http://example.test/v1");
    }

    #[test]
    fn the_judge_name_identifies_the_model() {
        let judge = OpenAiCompatibleJudge::ollama("qwen3.6:latest").expect("should build");

        assert_eq!(judge.name(), "llm:qwen3.6:latest");
    }

    #[test]
    fn request_body_matches_the_openai_shape() {
        let request = ChatRequest {
            model: "qwen3.6:latest",
            stream: false,
            response_format: ResponseFormat { kind: "json_object" },
            messages: vec![ChatMessage {
                role: "user",
                content: "hello".to_string(),
            }],
        };

        let json = serde_json::to_value(&request).expect("should serialise");

        assert_eq!(json["model"], "qwen3.6:latest");
        assert_eq!(json["stream"], false);
        assert_eq!(json["response_format"]["type"], "json_object");
        assert_eq!(json["messages"][0]["role"], "user");
    }
}
