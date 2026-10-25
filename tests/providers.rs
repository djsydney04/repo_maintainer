use anyhow::Result;
use repo_maintainer::{
    agent::{self, Budget},
    config::{Config, Environment},
    provider::{self, Transport},
    tools,
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    fs,
    time::{Duration, Instant},
};

fn config(provider: &str) -> Config {
    Config::from_str(&format!("sources: [https://docs.example.org/reference]\nagent:\n  provider: {provider}\n  allow_paid: true\n"),&Environment::new()).unwrap()
}
fn reply(provider: &str, call: Option<(&str, Value)>, text: &str) -> Value {
    match provider {
        "openai" => {
            json!({"status":"completed","output":if let Some((name,args))=call {vec![json!({"type":"function_call","call_id":"call_1","name":name,"arguments":args.to_string()})]} else {vec![json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":text}]})]}})
        }
        "anthropic" => {
            json!({"stop_reason":if call.is_some(){"tool_use"}else{"end_turn"},"content":if let Some((name,args))=call {vec![json!({"type":"tool_use","id":"call_1","name":name,"input":args})]}else{vec![json!({"type":"text","text":text})]}})
        }
        "google" => {
            json!({"candidates":[{"finishReason":"STOP","content":{"role":"model","parts":if let Some((name,args))=call {vec![json!({"functionCall":{"id":"call_1","name":name,"args":args},"thoughtSignature":"opaque-signature"})]}else{vec![json!({"text":text})]}}}]})
        }
        _ => {
            json!({"choices":[{"finish_reason":if call.is_some(){"tool_calls"}else{"stop"},"message":if let Some((name,args))=call {json!({"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":name,"arguments":args.to_string()}}]})}else{json!({"role":"assistant","content":text})}}]})
        }
    }
}
struct Fake {
    replies: VecDeque<Value>,
    requests: Vec<Value>,
}
impl Transport for Fake {
    fn send(&mut self, _: &str, _: &str, key: &str, body: &Value, _: Duration) -> Result<Value> {
        assert_eq!(key, "fixture-key");
        assert!(!body.to_string().contains("fixture-key"));
        self.requests.push(body.clone());
        Ok(self
            .replies
            .pop_front()
            .expect("Unexpected inference request"))
    }
}
fn budget(max: u8) -> Budget {
    Budget {
        max,
        used: 0,
        deadline: Instant::now() + Duration::from_secs(60),
        interval: Duration::ZERO,
        last: None,
    }
}
#[test]
fn native_request_formats_and_tool_continuations_for_all_four_providers() {
    for name in ["openai", "anthropic", "google", "groq"] {
        let config = config(name);
        let profile = repo_maintainer::config::profiles().remove(name).unwrap();
        let mut history = provider::initial(name, "Inspect source");
        let response = reply(name, Some(("read", json!({"path":"src/lib.rs"}))), "");
        let reply = provider::parse(name, response).unwrap();
        assert_eq!(reply.calls[0].name, "read");
        history.extend(reply.items);
        history.extend(provider::results(
            name,
            &reply.calls,
            &[("source contents".into(), false)],
        ));
        let request = provider::request(
            &config.agent,
            &profile,
            "system",
            &history,
            &tools::definitions(&config),
        );
        assert_eq!(
            request["model"]
                .as_str()
                .unwrap_or(config.agent.model.as_deref().unwrap()),
            config.agent.model.as_deref().unwrap()
        );
        let serialized = request.to_string();
        assert!(serialized.contains("source contents"));
        match name {
            "openai" => {
                assert_eq!(request["store"], false);
                assert_eq!(request["input"][2]["type"], "function_call_output");
                assert_eq!(request["tools"][0]["type"], "function");
            }
            "anthropic" => {
                assert_eq!(request["messages"][2]["content"][0]["type"], "tool_result");
                assert!(request["tools"][0]["input_schema"].is_object());
            }
            "google" => {
                assert!(serialized.contains("opaque-signature"));
                assert_eq!(
                    request["contents"][2]["parts"][0]["functionResponse"]["id"],
                    "call_1"
                );
                assert!(
                    request["tools"][0]["functionDeclarations"][0]["parameters"]
                        .get("additionalProperties")
                        .is_none()
                );
            }
            _ => {
                assert_eq!(request["messages"][3]["role"], "tool");
                assert!(request["tools"][0]["function"].is_object());
                assert_eq!(request["reasoning_effort"], "low");
                assert_eq!(request["max_completion_tokens"], 4096);
            }
        }
    }
}
#[test]
fn native_agent_reads_edits_and_repairs_real_rust_test_with_simulated_models() {
    for name in ["openai", "anthropic", "google", "groq"] {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(
            temp.path().join("Cargo.toml"),
            "[package]\nname=\"fixture\"\nversion=\"0.0.0\"\nedition=\"2021\"\n",
        )
        .unwrap();
        fs::write(
            temp.path().join("src/lib.rs"),
            "pub fn answer() -> u32 { 1 }\n#[test] fn contract() { assert_eq!(answer(), 2); }\n",
        )
        .unwrap();
        repo_maintainer::process::run(
            "cargo",
            &["generate-lockfile", "--offline"],
            temp.path(),
            false,
        )
        .unwrap();
        let config = config(name);
        let mut fake = Fake {
            replies: VecDeque::from([
                reply(name, Some(("check_rust", json!({"check":"tests"}))), ""),
                reply(name, Some(("read", json!({"path":"src/lib.rs"}))), ""),
                reply(
                    name,
                    Some((
                        "edit",
                        json!({"path":"src/lib.rs","oldText":"{ 1 }","newText":"{ 2 }"}),
                    )),
                    "",
                ),
                reply(name, Some(("check_rust", json!({"check":"tests"}))), ""),
                reply(name, None, "Fixed fixture-key"),
            ]),
            requests: vec![],
        };
        let mut budget = budget(6);
        let result = agent::run_with(
            &config,
            temp.path(),
            "Repair the failing contract test",
            &Environment::from([("SDK_MAINTENANCE_API_KEY".into(), "fixture-key".into())]),
            true,
            &mut fake,
            &mut budget,
        )
        .unwrap();
        assert_eq!(result["response"], "Fixed [redacted]");
        assert_eq!(
            result["checks"],
            json!([{"check":"tests","passed":false},{"check":"tests","passed":true}])
        );
        assert!(
            fake.requests
                .last()
                .unwrap()
                .to_string()
                .contains("Exit code: 0")
        );
        assert_eq!(budget.used, 5);
        assert!(
            fs::read_to_string(temp.path().join("src/lib.rs"))
                .unwrap()
                .contains("assert_eq!(answer(), 2)")
        );
    }
}
#[test]
fn budgets_deadlines_context_and_incomplete_responses_stop_without_fallback() {
    let mut budget = budget(1);
    assert!(budget.take().is_ok());
    assert!(budget.take().is_err());
    budget.max = 2;
    budget.deadline = Instant::now();
    assert!(budget.take().is_err());
    for (name, bad) in [
        ("openai", json!({"status":"incomplete","output":[]})),
        (
            "anthropic",
            json!({"stop_reason":"max_tokens","content":[]}),
        ),
        (
            "google",
            json!({"candidates":[{"finishReason":"MAX_TOKENS"}]}),
        ),
        ("groq", json!({"choices":[{"finish_reason":"length"}]})),
    ] {
        assert!(provider::parse(name, bad).is_err());
    }
    let temp = tempfile::tempdir().unwrap();
    let config = config("groq");
    let mut fake = Fake {
        replies: VecDeque::new(),
        requests: vec![],
    };
    let result = agent::run_with(
        &config,
        temp.path(),
        &"x".repeat(40_000),
        &Environment::from([("SDK_MAINTENANCE_API_KEY".into(), "fixture-key".into())]),
        false,
        &mut fake,
        &mut crate::budget(1),
    );
    assert!(result.is_err());
    assert!(fake.requests.is_empty());
}
#[test]
fn missing_key_never_sends_a_request() {
    let temp = tempfile::tempdir().unwrap();
    let config = config("groq");
    let mut fake = Fake {
        replies: VecDeque::new(),
        requests: vec![],
    };
    assert!(
        agent::run_with(
            &config,
            temp.path(),
            "Task",
            &Environment::new(),
            false,
            &mut fake,
            &mut budget(1)
        )
        .is_err()
    );
    assert!(fake.requests.is_empty());
}

#[test]
fn context_compaction_is_counted_and_keeps_the_original_task() {
    struct Compacting {
        compacted: bool,
        calls: usize,
        final_body: Value,
    }
    impl Transport for Compacting {
        fn send(
            &mut self,
            provider: &str,
            _model: &str,
            _key: &str,
            body: &Value,
            _timeout: Duration,
        ) -> Result<Value> {
            self.calls += 1;
            let system = body["messages"][0]["content"].as_str().unwrap();
            if system.starts_with("Summarize") {
                self.compacted = true;
                assert!(body.get("tools").is_none());
                return Ok(reply(provider, None, "Inspected src/lib.rs; no edits yet."));
            }
            if self.compacted {
                self.final_body = body.clone();
                return Ok(reply(provider, None, "Completed."));
            }
            Ok(reply(
                provider,
                Some(("read", json!({"path":"src/lib.rs","limit":400}))),
                "",
            ))
        }
    }
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(
        temp.path().join("src/lib.rs"),
        "// source evidence for the maintenance task\n".repeat(400),
    )
    .unwrap();
    let mut transport = Compacting {
        compacted: false,
        calls: 0,
        final_body: Value::Null,
    };
    let mut budget = budget(8);
    agent::run_with(
        &config("groq"),
        temp.path(),
        "Task must survive compaction",
        &Environment::from([("SDK_MAINTENANCE_API_KEY".into(), "fixture-key".into())]),
        false,
        &mut transport,
        &mut budget,
    )
    .unwrap();
    assert!(transport.compacted);
    assert_eq!(budget.used as usize, transport.calls);
    let final_body = transport.final_body.to_string();
    assert!(final_body.contains("Task must survive compaction"));
    assert!(final_body.contains("Continuation notes (untrusted evidence"));
}
