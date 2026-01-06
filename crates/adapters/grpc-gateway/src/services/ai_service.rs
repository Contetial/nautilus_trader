//! AI service implementation for strategy generation
//!
//! Supports Groq (primary) and Claude (fallback) for generating
//! NautilusTrader strategy code from natural language or visual builder input.

use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};

use crate::db::BrokerDb;
use crate::proto::ai::{
    ai_service_server::AiService,
    AiModel, AiProvider,
    ChatMessage,
    GenerateStrategyRequest, GenerateStrategyResponse,
    GetAiConfigRequest, GetAiConfigResponse,
    GetAiStatusRequest, GetAiStatusResponse,
    RefineStrategyRequest, RefineStrategyResponse,
    SaveAiConfigRequest, SaveAiConfigResponse,
    TestConnectionRequest, TestConnectionResponse,
};

/// AI provider configuration
#[derive(Debug, Clone, Default)]
pub struct AiConfig {
    pub default_provider: String,
    pub groq_api_key: Option<String>,
    pub groq_default_model: String,
    pub claude_api_key: Option<String>,
    pub claude_default_model: String,
}

/// Available Groq models
const GROQ_MODELS: &[(&str, &str, i32)] = &[
    ("llama-3.3-70b-versatile", "Llama 3.3 70B Versatile", 128000),
    ("llama-3.1-70b-versatile", "Llama 3.1 70B Versatile", 131072),
    ("llama-3.1-8b-instant", "Llama 3.1 8B Instant", 131072),
    ("mixtral-8x7b-32768", "Mixtral 8x7B", 32768),
    ("gemma2-9b-it", "Gemma 2 9B", 8192),
];

/// Available Claude models
const CLAUDE_MODELS: &[(&str, &str, i32)] = &[
    ("claude-sonnet-4-20250514", "Claude 4 Sonnet", 200000),
    ("claude-3-5-sonnet-20241022", "Claude 3.5 Sonnet", 200000),
    ("claude-3-5-haiku-20241022", "Claude 3.5 Haiku", 200000),
];

/// NautilusTrader strategy prompt template
const STRATEGY_PROMPT_TEMPLATE: &str = r#"You are an expert NautilusTrader strategy developer. Generate a complete, runnable Python strategy class based on the user's requirements.

IMPORTANT REQUIREMENTS:
1. The strategy MUST inherit from `nautilus_trader.trading.strategy.Strategy`
2. Use proper NautilusTrader imports and types
3. Include proper docstrings and type hints
4. Implement all required methods: __init__, on_start, on_stop, on_bar, on_quote (as needed)
5. Use indicator classes from `nautilus_trader.indicators`
6. Handle order management properly with order IDs and proper lifecycle
7. Include risk management (position sizing, stop loss, take profit) if mentioned
8. Generate production-ready code, not pseudo-code

OUTPUT FORMAT:
Return ONLY the Python code without any markdown formatting or explanation.
The code should be complete and runnable.

USER REQUEST:
{user_prompt}

{visual_builder_context}
"#;

/// AI Service implementation
pub struct AiServiceImpl {
    config: Arc<RwLock<AiConfig>>,
    http_client: reqwest::Client,
    db: Option<BrokerDb>,
}

impl std::fmt::Debug for AiServiceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiServiceImpl").finish()
    }
}

/// Get the default database path (~/.nautilus/brokers.db)
fn get_default_db_path() -> anyhow::Result<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("Failed to determine home directory"))?;
    Ok(std::path::PathBuf::from(home)
        .join(".nautilus")
        .join("brokers.db"))
}

impl AiServiceImpl {
    pub fn new() -> Self {
        // Try to initialize database and load saved config
        let (db, initial_config) = match get_default_db_path().and_then(|path| BrokerDb::new(&path)) {
            Ok(db) => {
                // Try to load saved config from database
                let config = match db.get_all_ai_config() {
                    Ok(saved) => {
                        tracing::info!("Loading AI config from database ({} keys)", saved.len());
                        AiConfig {
                            default_provider: saved.get("default_provider")
                                .cloned()
                                .unwrap_or_else(|| "groq".to_string()),
                            groq_api_key: saved.get("groq_api_key").cloned(),
                            groq_default_model: saved.get("groq_default_model")
                                .cloned()
                                .unwrap_or_else(|| "llama-3.3-70b-versatile".to_string()),
                            claude_api_key: saved.get("claude_api_key").cloned(),
                            claude_default_model: saved.get("claude_default_model")
                                .cloned()
                                .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load AI config from database: {}", e);
                        AiConfig {
                            default_provider: "groq".to_string(),
                            groq_default_model: "llama-3.3-70b-versatile".to_string(),
                            claude_default_model: "claude-sonnet-4-20250514".to_string(),
                            ..Default::default()
                        }
                    }
                };
                (Some(db), config)
            }
            Err(e) => {
                tracing::warn!("Failed to initialize AI config database: {}", e);
                (None, AiConfig {
                    default_provider: "groq".to_string(),
                    groq_default_model: "llama-3.3-70b-versatile".to_string(),
                    claude_default_model: "claude-sonnet-4-20250514".to_string(),
                    ..Default::default()
                })
            }
        };

        if initial_config.groq_api_key.is_some() {
            tracing::info!("Groq API key loaded from database");
        }
        if initial_config.claude_api_key.is_some() {
            tracing::info!("Claude API key loaded from database");
        }

        Self {
            config: Arc::new(RwLock::new(initial_config)),
            http_client: reqwest::Client::new(),
            db,
        }
    }

    /// Generate strategy using Groq API
    async fn generate_with_groq(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        history: &[ChatMessage],
    ) -> Result<(String, i32), String> {
        let mut messages: Vec<serde_json::Value> = vec![
            serde_json::json!({
                "role": "system",
                "content": "You are an expert NautilusTrader strategy developer. Generate complete, production-ready Python strategy code."
            })
        ];

        // Add conversation history
        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        // Add current prompt
        messages.push(serde_json::json!({
            "role": "user",
            "content": prompt
        }));

        let response = self.http_client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": model,
                "messages": messages,
                "temperature": 0.7,
                "max_tokens": 8192
            }))
            .send()
            .await
            .map_err(|e| format!("Groq API request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Groq API error: {}", error_text));
        }

        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("No content in Groq response")?
            .to_string();

        let tokens = json["usage"]["total_tokens"]
            .as_i64()
            .unwrap_or(0) as i32;

        Ok((content, tokens))
    }

    /// Generate strategy using Claude API
    async fn generate_with_claude(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        history: &[ChatMessage],
    ) -> Result<(String, i32), String> {
        let mut messages: Vec<serde_json::Value> = Vec::new();

        // Add conversation history
        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        // Add current prompt
        messages.push(serde_json::json!({
            "role": "user",
            "content": prompt
        }));

        let response = self.http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": 8192,
                "system": "You are an expert NautilusTrader strategy developer. Generate complete, production-ready Python strategy code.",
                "messages": messages
            }))
            .send()
            .await
            .map_err(|e| format!("Claude API request failed: {}", e))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(format!("Claude API error: {}", error_text));
        }

        let json: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Claude response: {}", e))?;

        let content = json["content"][0]["text"]
            .as_str()
            .ok_or("No content in Claude response")?
            .to_string();

        let input_tokens = json["usage"]["input_tokens"].as_i64().unwrap_or(0);
        let output_tokens = json["usage"]["output_tokens"].as_i64().unwrap_or(0);
        let tokens = (input_tokens + output_tokens) as i32;

        Ok((content, tokens))
    }

    /// Build the full prompt from user input
    fn build_prompt(&self, user_prompt: &str, visual_builder_json: &str, input_mode: &str) -> String {
        let visual_context = if !visual_builder_json.is_empty() && input_mode != "natural_language" {
            format!("\nVISUAL BUILDER CONFIGURATION:\n{}\n\nGenerate a strategy that implements these visual builder conditions.", visual_builder_json)
        } else {
            String::new()
        };

        STRATEGY_PROMPT_TEMPLATE
            .replace("{user_prompt}", user_prompt)
            .replace("{visual_builder_context}", &visual_context)
    }

    /// Extract strategy name from generated code
    fn extract_strategy_name(code: &str) -> String {
        // Look for class definition
        for line in code.lines() {
            if line.starts_with("class ") && line.contains("(Strategy)") {
                if let Some(name) = line.strip_prefix("class ") {
                    if let Some(idx) = name.find('(') {
                        return name[..idx].trim().to_string();
                    }
                }
            }
        }
        "GeneratedStrategy".to_string()
    }

    /// Clean up generated code (remove markdown formatting if present)
    fn clean_code(code: &str) -> String {
        let code = code.trim();

        // Remove markdown code blocks if present
        if code.starts_with("```python") {
            let code = code.strip_prefix("```python").unwrap_or(code);
            if let Some(idx) = code.rfind("```") {
                return code[..idx].trim().to_string();
            }
        }

        if code.starts_with("```") {
            let code = code.strip_prefix("```").unwrap_or(code);
            if let Some(idx) = code.rfind("```") {
                return code[..idx].trim().to_string();
            }
        }

        code.to_string()
    }

    /// Mask API key for display
    fn mask_api_key(key: &str) -> String {
        if key.len() <= 8 {
            return "*".repeat(key.len());
        }
        format!("{}...{}", &key[..4], &key[key.len()-4..])
    }
}

impl Default for AiServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl AiService for AiServiceImpl {
    async fn generate_strategy(
        &self,
        request: Request<GenerateStrategyRequest>,
    ) -> Result<Response<GenerateStrategyResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("Generating strategy with mode: {}", req.input_mode);

        let config = self.config.read().await;

        // Determine provider and model
        let provider = if req.provider.is_empty() {
            config.default_provider.clone()
        } else {
            req.provider.clone()
        };

        // Get API key and model for selected provider
        let (api_key, model) = match provider.as_str() {
            "groq" => {
                let key = config.groq_api_key.clone()
                    .ok_or_else(|| Status::failed_precondition("Groq API key not configured"))?;
                let model = if req.model.is_empty() {
                    config.groq_default_model.clone()
                } else {
                    req.model.clone()
                };
                (key, model)
            }
            "claude" => {
                let key = config.claude_api_key.clone()
                    .ok_or_else(|| Status::failed_precondition("Claude API key not configured"))?;
                let model = if req.model.is_empty() {
                    config.claude_default_model.clone()
                } else {
                    req.model.clone()
                };
                (key, model)
            }
            _ => {
                return Err(Status::invalid_argument(format!("Unknown provider: {}", provider)));
            }
        };

        drop(config); // Release lock before async calls

        // Build the prompt
        let prompt = self.build_prompt(&req.prompt, &req.visual_builder_json, &req.input_mode);

        // Generate with selected provider
        let result = match provider.as_str() {
            "groq" => {
                self.generate_with_groq(&api_key, &model, &prompt, &req.history).await
            }
            "claude" => {
                self.generate_with_claude(&api_key, &model, &prompt, &req.history).await
            }
            _ => unreachable!()
        };

        match result {
            Ok((code, tokens)) => {
                let cleaned_code = Self::clean_code(&code);
                let strategy_name = Self::extract_strategy_name(&cleaned_code);

                Ok(Response::new(GenerateStrategyResponse {
                    success: true,
                    strategy_code: cleaned_code,
                    strategy_name,
                    error: String::new(),
                    provider_used: provider,
                    model_used: model,
                    tokens_used: tokens,
                }))
            }
            Err(e) => {
                tracing::error!("Strategy generation failed: {}", e);
                Ok(Response::new(GenerateStrategyResponse {
                    success: false,
                    strategy_code: String::new(),
                    strategy_name: String::new(),
                    error: e,
                    provider_used: provider,
                    model_used: model,
                    tokens_used: 0,
                }))
            }
        }
    }

    async fn refine_strategy(
        &self,
        request: Request<RefineStrategyRequest>,
    ) -> Result<Response<RefineStrategyResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("Refining strategy");

        let config = self.config.read().await;

        let provider = if req.provider.is_empty() {
            config.default_provider.clone()
        } else {
            req.provider.clone()
        };

        let (api_key, model) = match provider.as_str() {
            "groq" => {
                let key = config.groq_api_key.clone()
                    .ok_or_else(|| Status::failed_precondition("Groq API key not configured"))?;
                (key, config.groq_default_model.clone())
            }
            "claude" => {
                let key = config.claude_api_key.clone()
                    .ok_or_else(|| Status::failed_precondition("Claude API key not configured"))?;
                (key, config.claude_default_model.clone())
            }
            _ => {
                return Err(Status::invalid_argument(format!("Unknown provider: {}", provider)));
            }
        };

        drop(config);

        let prompt = format!(
            "Here is the current strategy code:\n\n```python\n{}\n```\n\nPlease apply these refinements:\n{}\n\nReturn the complete updated strategy code.",
            req.current_code,
            req.instructions
        );

        let result = match provider.as_str() {
            "groq" => self.generate_with_groq(&api_key, &model, &prompt, &req.history).await,
            "claude" => self.generate_with_claude(&api_key, &model, &prompt, &req.history).await,
            _ => unreachable!()
        };

        match result {
            Ok((code, tokens)) => {
                let cleaned_code = Self::clean_code(&code);
                Ok(Response::new(RefineStrategyResponse {
                    success: true,
                    refined_code: cleaned_code,
                    changes_summary: "Strategy refined successfully".to_string(),
                    error: String::new(),
                    provider_used: provider,
                    tokens_used: tokens,
                }))
            }
            Err(e) => {
                Ok(Response::new(RefineStrategyResponse {
                    success: false,
                    refined_code: String::new(),
                    changes_summary: String::new(),
                    error: e,
                    provider_used: provider,
                    tokens_used: 0,
                }))
            }
        }
    }

    async fn get_ai_status(
        &self,
        _request: Request<GetAiStatusRequest>,
    ) -> Result<Response<GetAiStatusResponse>, Status> {
        let config = self.config.read().await;

        let groq_provider = AiProvider {
            id: "groq".to_string(),
            name: "Groq".to_string(),
            configured: config.groq_api_key.is_some(),
            available: config.groq_api_key.is_some(),
            models: GROQ_MODELS.iter().map(|(id, name, ctx)| AiModel {
                id: id.to_string(),
                name: name.to_string(),
                recommended: *id == "llama-3.3-70b-versatile",
                context_length: *ctx,
            }).collect(),
        };

        let claude_provider = AiProvider {
            id: "claude".to_string(),
            name: "Claude".to_string(),
            configured: config.claude_api_key.is_some(),
            available: config.claude_api_key.is_some(),
            models: CLAUDE_MODELS.iter().map(|(id, name, ctx)| AiModel {
                id: id.to_string(),
                name: name.to_string(),
                recommended: *id == "claude-sonnet-4-20250514",
                context_length: *ctx,
            }).collect(),
        };

        Ok(Response::new(GetAiStatusResponse {
            providers: vec![groq_provider, claude_provider],
            default_provider: config.default_provider.clone(),
        }))
    }

    async fn save_ai_config(
        &self,
        request: Request<SaveAiConfigRequest>,
    ) -> Result<Response<SaveAiConfigResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("Saving AI config for provider: {}", req.provider);

        let mut config = self.config.write().await;

        match req.provider.as_str() {
            "groq" => {
                if !req.api_key.is_empty() {
                    config.groq_api_key = Some(req.api_key.clone());
                    // Persist to database
                    if let Some(db) = &self.db {
                        if let Err(e) = db.save_ai_config("groq_api_key", &req.api_key) {
                            tracing::error!("Failed to persist Groq API key: {}", e);
                        }
                    }
                }
                if !req.default_model.is_empty() {
                    config.groq_default_model = req.default_model.clone();
                    if let Some(db) = &self.db {
                        let _ = db.save_ai_config("groq_default_model", &req.default_model);
                    }
                }
                config.default_provider = "groq".to_string();
                if let Some(db) = &self.db {
                    let _ = db.save_ai_config("default_provider", "groq");
                }
            }
            "claude" => {
                if !req.api_key.is_empty() {
                    config.claude_api_key = Some(req.api_key.clone());
                    if let Some(db) = &self.db {
                        if let Err(e) = db.save_ai_config("claude_api_key", &req.api_key) {
                            tracing::error!("Failed to persist Claude API key: {}", e);
                        }
                    }
                }
                if !req.default_model.is_empty() {
                    config.claude_default_model = req.default_model.clone();
                    if let Some(db) = &self.db {
                        let _ = db.save_ai_config("claude_default_model", &req.default_model);
                    }
                }
                config.default_provider = "claude".to_string();
                if let Some(db) = &self.db {
                    let _ = db.save_ai_config("default_provider", "claude");
                }
            }
            _ => {
                return Ok(Response::new(SaveAiConfigResponse {
                    success: false,
                    error: format!("Unknown provider: {}", req.provider),
                }));
            }
        }

        tracing::info!("AI config saved successfully for provider: {}", req.provider);
        Ok(Response::new(SaveAiConfigResponse {
            success: true,
            error: String::new(),
        }))
    }

    async fn get_ai_config(
        &self,
        _request: Request<GetAiConfigRequest>,
    ) -> Result<Response<GetAiConfigResponse>, Status> {
        let config = self.config.read().await;

        Ok(Response::new(GetAiConfigResponse {
            default_provider: config.default_provider.clone(),
            groq_configured: config.groq_api_key.is_some(),
            groq_api_key_masked: config.groq_api_key.as_ref()
                .map(|k| Self::mask_api_key(k))
                .unwrap_or_default(),
            groq_default_model: config.groq_default_model.clone(),
            claude_configured: config.claude_api_key.is_some(),
            claude_api_key_masked: config.claude_api_key.as_ref()
                .map(|k| Self::mask_api_key(k))
                .unwrap_or_default(),
            claude_default_model: config.claude_default_model.clone(),
        }))
    }

    async fn test_connection(
        &self,
        request: Request<TestConnectionRequest>,
    ) -> Result<Response<TestConnectionResponse>, Status> {
        let req = request.into_inner();
        tracing::info!("Testing AI connection for provider: {}", req.provider);

        match req.provider.as_str() {
            "groq" => {
                // Test Groq connection by listing models
                let response = self.http_client
                    .get("https://api.groq.com/openai/v1/models")
                    .header("Authorization", format!("Bearer {}", req.api_key))
                    .send()
                    .await;

                match response {
                    Ok(resp) if resp.status().is_success() => {
                        let json: serde_json::Value = resp.json().await.unwrap_or_default();
                        let models: Vec<String> = json["data"]
                            .as_array()
                            .map(|arr| arr.iter()
                                .filter_map(|m| m["id"].as_str().map(String::from))
                                .collect())
                            .unwrap_or_default();

                        Ok(Response::new(TestConnectionResponse {
                            success: true,
                            error: String::new(),
                            available_models: models,
                        }))
                    }
                    Ok(resp) => {
                        let error = resp.text().await.unwrap_or_default();
                        Ok(Response::new(TestConnectionResponse {
                            success: false,
                            error: format!("API error: {}", error),
                            available_models: vec![],
                        }))
                    }
                    Err(e) => {
                        Ok(Response::new(TestConnectionResponse {
                            success: false,
                            error: format!("Connection failed: {}", e),
                            available_models: vec![],
                        }))
                    }
                }
            }
            "claude" => {
                // Test Claude connection with a simple message
                let response = self.http_client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", &req.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": "claude-3-5-haiku-20241022",
                        "max_tokens": 10,
                        "messages": [{"role": "user", "content": "Hi"}]
                    }))
                    .send()
                    .await;

                match response {
                    Ok(resp) if resp.status().is_success() => {
                        Ok(Response::new(TestConnectionResponse {
                            success: true,
                            error: String::new(),
                            available_models: CLAUDE_MODELS.iter()
                                .map(|(id, _, _)| id.to_string())
                                .collect(),
                        }))
                    }
                    Ok(resp) => {
                        let error = resp.text().await.unwrap_or_default();
                        Ok(Response::new(TestConnectionResponse {
                            success: false,
                            error: format!("API error: {}", error),
                            available_models: vec![],
                        }))
                    }
                    Err(e) => {
                        Ok(Response::new(TestConnectionResponse {
                            success: false,
                            error: format!("Connection failed: {}", e),
                            available_models: vec![],
                        }))
                    }
                }
            }
            _ => {
                Ok(Response::new(TestConnectionResponse {
                    success: false,
                    error: format!("Unknown provider: {}", req.provider),
                    available_models: vec![],
                }))
            }
        }
    }
}
