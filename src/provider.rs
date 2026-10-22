use crate::{
    config::{Agent, Profile},
    tools::Definition,
};
use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

pub struct Call {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
pub struct Reply {
    pub items: Vec<Value>,
    pub calls: Vec<Call>,
    pub text: String,
}
pub fn initial(provider: &str, prompt: &str) -> Vec<Value> {
    if provider == "google" {
        vec![json!({"role":"user","parts":[{"text":prompt}]})]
    } else {
        vec![json!({"role":"user","content":prompt})]
    }
}
pub fn request(
    agent: &Agent,
    profile: &Profile,
    system: &str,
    history: &[Value],
    tools: &[Definition],
) -> Value {
    let model = agent.model.as_ref().unwrap();
    match agent.provider.as_str() {
        "openai" => {
            json!({"model":model,"instructions":system,"input":history,"store":false,"include":["reasoning.encrypted_content"],"max_output_tokens":profile.max_tokens,
            "tools":tools.iter().map(|t|json!({"type":"function","name":t.name,"description":t.description,"parameters":t.parameters,"strict":false})).collect::<Vec<_>>() })
        }
        "anthropic" => {
            json!({"model":model,"system":system,"messages":history,"max_tokens":profile.max_tokens,
            "tools":tools.iter().map(|t|json!({"name":t.name,"description":t.description,"input_schema":t.parameters})).collect::<Vec<_>>() })
        }
        "google" => {
            let mut body = json!({"systemInstruction":{"parts":[{"text":system}]},"contents":history,"generationConfig":{"maxOutputTokens":profile.max_tokens}});
            if !tools.is_empty() {
                body["tools"] = json!([{"functionDeclarations":tools.iter().map(|t| {
                let mut schema = t.parameters.clone(); schema.as_object_mut().unwrap().remove("additionalProperties");
                json!({"name":t.name,"description":t.description,"parameters":schema})
            }).collect::<Vec<_>>()}]);
            }
            body
        }
        _ => {
            let mut messages = vec![json!({"role":"system","content":system})];
            messages.extend_from_slice(history);
            let mut body = json!({"model":model,"messages":messages,"max_completion_tokens":profile.max_tokens});
            if !tools.is_empty() {
                body["tools"] = json!(tools.iter().map(|t|json!({"type":"function","function":{"name":t.name,"description":t.description,"parameters":t.parameters}})).collect::<Vec<_>>());
            }
            body
        }
    }
}
pub fn parse(provider: &str, response: Value) -> Result<Reply> {
    let mut calls = vec![];
    let mut text = String::new();
    let items = match provider {
        "openai" => {
            ensure!(
                response["status"] == "completed",
                "OpenAI response was incomplete or failed"
            );
            let items = response["output"]
                .as_array()
                .context("Missing Responses output")?
                .clone();
            for item in &items {
                if item["type"] == "function_call" {
                    calls.push(Call {
                        id: item["call_id"].as_str().context("Missing call ID")?.into(),
                        name: item["name"].as_str().context("Missing tool name")?.into(),
                        arguments: serde_json::from_str(
                            item["arguments"].as_str().context("Missing arguments")?,
                        )?,
                    });
                } else if item["type"] == "message" {
                    for part in item["content"]
                        .as_array()
                        .context("Missing message content")?
                    {
                        ensure!(
                            part["type"] != "refusal",
                            "OpenAI refused the maintenance request"
                        );
                        if part["type"] == "output_text" {
                            text.push_str(part["text"].as_str().unwrap_or(""));
                        }
                    }
                }
            }
            items
        }
        "anthropic" => {
            ensure!(
                matches!(
                    response["stop_reason"].as_str(),
                    Some("end_turn" | "tool_use")
                ),
                "Anthropic response was incomplete or refused"
            );
            let content = response["content"]
                .as_array()
                .context("Missing Messages content")?;
            for item in content {
                if item["type"] == "tool_use" {
                    calls.push(Call {
                        id: item["id"].as_str().context("Missing call ID")?.into(),
                        name: item["name"].as_str().context("Missing tool name")?.into(),
                        arguments: item["input"].clone(),
                    });
                }
                if item["type"] == "text" {
                    text.push_str(item["text"].as_str().unwrap_or(""));
                }
            }
            vec![json!({"role":"assistant","content":content})]
        }
        "google" => {
            let candidate = &response["candidates"][0];
            ensure!(
                candidate["finishReason"] == "STOP",
                "Gemini response was incomplete or blocked"
            );
            let content = candidate["content"].clone();
            for (i, part) in content["parts"]
                .as_array()
                .context("Missing Gemini content")?
                .iter()
                .enumerate()
            {
                if let Some(call) = part.get("functionCall") {
                    calls.push(Call {
                        id: call["id"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| i.to_string()),
                        name: call["name"].as_str().context("Missing tool name")?.into(),
                        arguments: call.get("args").cloned().unwrap_or(json!({})),
                    });
                }
                if part["thought"] != true
                    && let Some(s) = part["text"].as_str()
                {
                    text.push_str(s);
                }
            }
            // Preserve native thought signatures and call IDs verbatim.
            vec![content]
        }
        _ => {
            let choice = &response["choices"][0];
            ensure!(
                matches!(
                    choice["finish_reason"].as_str(),
                    Some("stop" | "tool_calls")
                ),
                "Groq response was incomplete or blocked"
            );
            let message = choice["message"].clone();
            if let Some(list) = message["tool_calls"].as_array() {
                for item in list {
                    calls.push(Call {
                        id: item["id"].as_str().context("Missing call ID")?.into(),
                        name: item["function"]["name"]
                            .as_str()
                            .context("Missing tool name")?
                            .into(),
                        arguments: serde_json::from_str(
                            item["function"]["arguments"]
                                .as_str()
                                .context("Missing arguments")?,
                        )?,
                    });
                }
            }
            text.push_str(message["content"].as_str().unwrap_or(""));
            vec![message]
        }
    };
    ensure!(calls.len() <= 16, "Too many tools in one response");
    let mut ids = std::collections::HashSet::new();
    ensure!(
        calls.iter().all(|c| !c.id.is_empty() && ids.insert(&c.id)),
        "Duplicate or missing call IDs"
    );
    ensure!(
        !calls.is_empty() || !text.is_empty(),
        "Model returned no final result"
    );
    Ok(Reply { items, calls, text })
}
pub fn results(provider: &str, calls: &[Call], outputs: &[(String, bool)]) -> Vec<Value> {
    match provider {
        "openai" => calls
            .iter()
            .zip(outputs)
            .map(|(c, (s, _))| json!({"type":"function_call_output","call_id":c.id,"output":s}))
            .collect(),
        "anthropic" => vec![
            json!({"role":"user","content":calls.iter().zip(outputs).map(|(c,(s,error))|json!({"type":"tool_result","tool_use_id":c.id,"content":s,"is_error":error})).collect::<Vec<_>>()}),
        ],
        "google" => vec![
            json!({"role":"user","parts":calls.iter().zip(outputs).map(|(c,(s,error))| {
            // Old generateContent calls may omit IDs; synthetic numeric IDs stay local.
            let mut result = json!({"name":c.name,"response":{"output":s,"isError":error}});
            if c.id.parse::<usize>().is_err() { result["id"] = json!(c.id); }
            json!({"functionResponse":result})
        }).collect::<Vec<_>>()}),
        ],
        _ => calls
            .iter()
            .zip(outputs)
            .map(|(c, (s, _))| json!({"role":"tool","tool_call_id":c.id,"content":s}))
            .collect(),
    }
}
pub trait Transport {
    fn send(
        &mut self,
        provider: &str,
        model: &str,
        key: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value>;
}
pub struct Http;
impl Transport for Http {
    fn send(
        &mut self,
        provider: &str,
        model: &str,
        key: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value> {
        let endpoint = match provider {
            "openai" => "https://api.openai.com/v1/responses".into(),
            "anthropic" => "https://api.anthropic.com/v1/messages".into(),
            "google" => {
                let model: String =
                    url::form_urlencoded::byte_serialize(model.as_bytes()).collect();
                format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
                )
            }
            "groq" => "https://api.groq.com/openai/v1/chat/completions".into(),
            _ => bail!("Unknown model provider"),
        };
        send_to(provider, key, body, timeout, &endpoint)
    }
}
fn send_to(
    provider: &str,
    key: &str,
    body: &Value,
    timeout: Duration,
    endpoint: &str,
) -> Result<Value> {
    let client = crate::network::trust(Client::builder())?
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let mut request = client.post(endpoint).json(body);
    request = match provider {
        "anthropic" => request
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        "google" => request.header("x-goog-api-key", key),
        _ => request.bearer_auth(key),
    };
    let response = request
        .send()
        .map_err(|_| anyhow::anyhow!("{provider} request failed; check network access"))?;
    ensure!(
        response.status().is_success(),
        "{provider} returned HTTP {}; check key, model access and quota (no automatic retry)",
        response.status().as_u16()
    );
    let mut bytes = vec![];
    response.take(2_000_001).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 2_000_000, "Model response exceeds 2 MB");
    serde_json::from_slice(&bytes).context("Invalid provider JSON response")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tiny_http::{Header, Response, Server};

    #[test]
    fn sends_native_authentication_headers_and_json_over_http() {
        for provider in ["openai", "anthropic", "google", "groq"] {
            let server = Server::http("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}/native", server.server_addr());
            let worker = thread::spawn(move || {
                let mut request = server
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap()
                    .unwrap();
                assert_eq!(request.url(), "/native");
                assert_eq!(request.method().as_str(), "POST");
                let header = |name: &'static str| {
                    request
                        .headers()
                        .iter()
                        .find(|h| h.field.equiv(name))
                        .map(|h| h.value.as_str().to_owned())
                };
                match provider {
                    "anthropic" => {
                        assert_eq!(header("x-api-key").as_deref(), Some("fixture-key"));
                        assert_eq!(header("anthropic-version").as_deref(), Some("2023-06-01"));
                        assert!(header("authorization").is_none());
                    }
                    "google" => {
                        assert_eq!(header("x-goog-api-key").as_deref(), Some("fixture-key"));
                        assert!(header("authorization").is_none());
                    }
                    _ => assert_eq!(
                        header("authorization").as_deref(),
                        Some("Bearer fixture-key")
                    ),
                }
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&body).unwrap(),
                    json!({"fixture":true})
                );
                request
                    .respond(Response::from_string("{\"ok\":true}"))
                    .unwrap();
            });
            let result = send_to(
                provider,
                "fixture-key",
                &json!({"fixture":true}),
                Duration::from_secs(10),
                &endpoint,
            )
            .unwrap();
            assert_eq!(result, json!({"ok":true}));
            worker.join().unwrap();
        }
    }
    #[test]
    fn provider_redirects_fail_without_leaking_response_body_or_key() {
        let server = Server::http("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/native", server.server_addr());
        let worker = thread::spawn(move || {
            let request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap();
            request
                .respond(
                    Response::from_string("sensitive-server-detail fixture-key")
                        .with_status_code(302)
                        .with_header(
                            Header::from_bytes("Location", "http://127.0.0.1:1/evil").unwrap(),
                        ),
                )
                .unwrap();
        });
        let error = send_to(
            "openai",
            "fixture-key",
            &json!({}),
            Duration::from_secs(10),
            &endpoint,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("302"));
        assert!(!error.contains("fixture-key"));
        assert!(!error.contains("sensitive-server-detail"));
        worker.join().unwrap();
    }
}
