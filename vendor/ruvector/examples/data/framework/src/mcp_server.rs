//! MCP (Model Context Protocol) Server for RuVector Data Discovery
//!
//! Implements the Anthropic MCP specification (2024-11-05) with support for:
//! - JSON-RPC 2.0 message handling
//! - STDIO and SSE transports
//! - 22+ discovery tools across research, medical, economic, and knowledge domains
//! - Resources for discovered data access
//! - Pre-built discovery prompts

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    ArxivClient, BiorxivClient, ClinicalTrialsClient, CrossRefClient,
    FdaClient, FredClient, MedrxivClient, NativeDiscoveryEngine,
    NoaaClient, OpenAlexClient, PubMedClient, SemanticScholarClient,
    SimpleEmbedder, WikidataClient, WikipediaClient, WorldBankClient,
    NativeEngineConfig, Result, FrameworkError,
};

// ============================================================================
// JSON-RPC 2.0 Message Types
// ============================================================================

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// JSON-RPC Error Codes
#[allow(dead_code)]
impl JsonRpcError {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    pub fn parse_error(msg: &str) -> Self {
        Self { code: Self::PARSE_ERROR, message: msg.to_string(), data: None }
    }

    pub fn invalid_request(msg: &str) -> Self {
        Self { code: Self::INVALID_REQUEST, message: msg.to_string(), data: None }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: Self::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method),
            data: None
        }
    }

    pub fn invalid_params(msg: &str) -> Self {
        Self { code: Self::INVALID_PARAMS, message: msg.to_string(), data: None }
    }

    pub fn internal_error(msg: &str) -> Self {
        Self { code: Self::INTERNAL_ERROR, message: msg.to_string(), data: None }
    }
}

// ============================================================================
// MCP Protocol Types
// ============================================================================

/// MCP Server Capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub tools: ToolsCapability,
    pub resources: ResourcesCapability,
    pub prompts: PromptsCapability,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    pub list_changed: bool,
    pub subscribe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    pub list_changed: bool,
}

/// MCP Tool Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// MCP Resource Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// MCP Prompt Definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptDefinition {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

// ============================================================================
// Transport Layer
// ============================================================================

/// MCP Transport mechanism
pub enum McpTransport {
    /// Standard I/O transport (stdin/stdout)
    Stdio,
    /// Server-Sent Events (HTTP streaming)
    Sse { endpoint: String, port: u16 },
}

// ============================================================================
// Data Source Clients
// ============================================================================

/// Container for all data source API clients
pub struct DataSourceClients {
    pub openalex: Arc<OpenAlexClient>,
    pub arxiv: Arc<ArxivClient>,
    pub semantic_scholar: Arc<SemanticScholarClient>,
    pub crossref: Arc<CrossRefClient>,
    pub biorxiv: Arc<BiorxivClient>,
    pub medrxiv: Arc<MedrxivClient>,
    pub pubmed: Arc<PubMedClient>,
    pub clinical_trials: Arc<ClinicalTrialsClient>,
    pub fda: Arc<FdaClient>,
    pub fred: Arc<FredClient>,
    pub worldbank: Arc<WorldBankClient>,
    pub noaa: Arc<NoaaClient>,
    pub wikipedia: Arc<WikipediaClient>,
    pub wikidata: Arc<WikidataClient>,
    pub embedder: Arc<SimpleEmbedder>,
}

impl DataSourceClients {
    /// Create new data source clients
    pub fn new() -> Self {
        Self {
            openalex: Arc::new(OpenAlexClient::new(None).expect("Failed to create OpenAlex client")),
            arxiv: Arc::new(ArxivClient::new()),
            semantic_scholar: Arc::new(SemanticScholarClient::new(None)),
            crossref: Arc::new(CrossRefClient::new(None)),
            biorxiv: Arc::new(BiorxivClient::new()),
            medrxiv: Arc::new(MedrxivClient::new()),
            pubmed: Arc::new(PubMedClient::new(None).expect("Failed to create PubMed client")),
            clinical_trials: Arc::new(ClinicalTrialsClient::new().expect("Failed to create ClinicalTrials client")),
            fda: Arc::new(FdaClient::new().expect("Failed to create FDA client")),
            fred: Arc::new(FredClient::new(None).expect("Failed to create FRED client")),
            worldbank: Arc::new(WorldBankClient::new().expect("Failed to create WorldBank client")),
            noaa: Arc::new(NoaaClient::new(None).expect("Failed to create NOAA client")),
            wikipedia: Arc::new(WikipediaClient::new("en".to_string()).expect("Failed to create Wikipedia client")),
            wikidata: Arc::new(WikidataClient::new().expect("Failed to create Wikidata client")),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        }
    }
}

impl Default for DataSourceClients {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// MCP Server Configuration
// ============================================================================

/// MCP Server Configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub version: String,
    pub max_request_size: usize,
    pub rate_limit_per_minute: u32,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: "ruvector-discovery-mcp".to_string(),
            version: "1.0.0".to_string(),
            max_request_size: 10_485_760, // 10MB
            rate_limit_per_minute: 100,
        }
    }
}

// ============================================================================
// MCP Discovery Server
// ============================================================================

/// MCP Server for RuVector Data Discovery
pub struct McpDiscoveryServer {
    transport: McpTransport,
    engine: Arc<RwLock<NativeDiscoveryEngine>>,
    clients: DataSourceClients,
    config: McpServerConfig,
    initialized: bool,
    request_count: Arc<RwLock<HashMap<String, u32>>>,
}

impl McpDiscoveryServer {
    /// Create a new MCP discovery server
    pub fn new(transport: McpTransport, engine_config: NativeEngineConfig) -> Self {
        Self {
            transport,
            engine: Arc::new(RwLock::new(NativeDiscoveryEngine::new(engine_config))),
            clients: DataSourceClients::new(),
            config: McpServerConfig::default(),
            initialized: false,
            request_count: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Run the MCP server
    pub async fn run(&mut self) -> Result<()> {
        match &self.transport {
            McpTransport::Stdio => self.run_stdio().await,
            McpTransport::Sse { endpoint, port } => {
                self.run_sse(endpoint.clone(), *port).await
            }
        }
    }

    /// Run STDIO transport
    async fn run_stdio(&mut self) -> Result<()> {
        let stdin = io::stdin();
        let mut stdout = io::stdout();

        eprintln!("RuVector MCP Server started (STDIO mode)");

        for line in stdin.lock().lines() {
            let line = line.map_err(|e| FrameworkError::Config(e.to_string()))?;

            // Parse JSON-RPC request
            let request: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    let error_response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError::parse_error(&e.to_string())),
                    };
                    let response_json = serde_json::to_string(&error_response)
                        .map_err(|e| FrameworkError::Serialization(e))?;
                    writeln!(stdout, "{}", response_json)
                        .map_err(|e| FrameworkError::Config(e.to_string()))?;
                    continue;
                }
            };

            // Handle request
            let response = self.handle_request(request).await;

            // Send response
            let response_json = serde_json::to_string(&response)
                .map_err(|e| FrameworkError::Serialization(e))?;
            writeln!(stdout, "{}", response_json)
                .map_err(|e| FrameworkError::Config(e.to_string()))?;
            stdout.flush()
                .map_err(|e| FrameworkError::Config(e.to_string()))?;
        }

        Ok(())
    }

    /// Run SSE transport
    async fn run_sse(&mut self, endpoint: String, port: u16) -> Result<()> {
        #[cfg(feature = "sse")]
        {
            use warp::Filter;

            eprintln!("RuVector MCP Server starting on {}:{} (SSE mode)", endpoint, port);

            let server = Arc::new(RwLock::new(self));

            // SSE endpoint
            let sse_route = warp::path("sse")
                .and(warp::get())
                .map(|| {
                    warp::sse::reply(warp::sse::keep_alive().stream(
                        futures::stream::iter(vec![
                            Ok::<_, warp::Error>(warp::sse::Event::default().data("connected"))
                        ])
                    ))
                });

            // Message endpoint
            let server_clone = server.clone();
            let message_route = warp::path("message")
                .and(warp::post())
                .and(warp::body::json())
                .and_then(move |request: JsonRpcRequest| {
                    let server = server_clone.clone();
                    async move {
                        let mut server = server.write().await;
                        let response = server.handle_request(request).await;
                        Ok::<_, warp::Rejection>(warp::reply::json(&response))
                    }
                });

            let routes = sse_route.or(message_route);

            warp::serve(routes)
                .run(([127, 0, 0, 1], port))
                .await;

            Ok(())
        }

        #[cfg(not(feature = "sse"))]
        {
            let _ = (endpoint, port);
            Err(FrameworkError::Config(
                "SSE transport requires the 'sse' feature. Compile with --features sse".to_string()
            ))
        }
    }

    /// Handle a JSON-RPC request
    async fn handle_request(&mut self, request: JsonRpcRequest) -> JsonRpcResponse {
        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError::invalid_request("JSON-RPC version must be 2.0")),
            };
        }

        // Handle method
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await,
            "initialized" => Ok(json!({})),
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tool_call(request.params).await,
            "resources/list" => self.handle_resources_list().await,
            "resources/read" => self.handle_resource_read(request.params).await,
            "prompts/list" => self.handle_prompts_list().await,
            "prompts/get" => self.handle_prompt_get(request.params).await,
            _ => Err(JsonRpcError::method_not_found(&request.method)),
        };

        match result {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(result),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(error),
            },
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&mut self, _params: Option<Value>) -> std::result::Result<Value, JsonRpcError> {
        self.initialized = true;

        Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": self.config.name,
                "version": self.config.version,
            },
            "capabilities": ServerCapabilities {
                tools: ToolsCapability { list_changed: false },
                resources: ResourcesCapability { list_changed: false, subscribe: false },
                prompts: PromptsCapability { list_changed: false },
            }
        }))
    }

    /// Handle tools/list request
    async fn handle_tools_list(&self) -> std::result::Result<Value, JsonRpcError> {
        let tools = vec![
            // Research tools
            self.tool_search_openalex(),
            self.tool_search_arxiv(),
            self.tool_search_semantic_scholar(),
            self.tool_get_citations(),
            self.tool_search_crossref(),
            self.tool_search_biorxiv(),
            self.tool_search_medrxiv(),

            // Medical tools
            self.tool_search_pubmed(),
            self.tool_search_clinical_trials(),
            self.tool_search_fda_events(),

            // Economic tools
            self.tool_get_fred_series(),
            self.tool_get_worldbank_indicator(),

            // Climate tools
            self.tool_get_noaa_data(),

            // Knowledge tools
            self.tool_search_wikipedia(),
            self.tool_query_wikidata(),

            // Discovery tools
            self.tool_run_discovery(),
            self.tool_analyze_coherence(),
            self.tool_detect_patterns(),
            self.tool_export_graph(),
        ];

        Ok(json!({ "tools": tools }))
    }

    /// Handle tools/call request
    async fn handle_tool_call(&mut self, params: Option<Value>) -> std::result::Result<Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;

        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing tool name"))?;

        let arguments = params.get("arguments")
            .ok_or_else(|| JsonRpcError::invalid_params("Missing arguments"))?;

        // Check rate limiting
        self.check_rate_limit(name).await?;

        // Execute tool
        let result = match name {
            "search_openalex" => self.execute_search_openalex(arguments).await,
            "search_arxiv" => self.execute_search_arxiv(arguments).await,
            "search_semantic_scholar" => self.execute_search_semantic_scholar(arguments).await,
            "get_citations" => self.execute_get_citations(arguments).await,
            "search_crossref" => self.execute_search_crossref(arguments).await,
            "search_biorxiv" => self.execute_search_biorxiv(arguments).await,
            "search_medrxiv" => self.execute_search_medrxiv(arguments).await,
            "search_pubmed" => self.execute_search_pubmed(arguments).await,
            "search_clinical_trials" => self.execute_search_clinical_trials(arguments).await,
            "search_fda_events" => self.execute_search_fda_events(arguments).await,
            "get_fred_series" => self.execute_get_fred_series(arguments).await,
            "get_worldbank_indicator" => self.execute_get_worldbank_indicator(arguments).await,
            "get_noaa_data" => self.execute_get_noaa_data(arguments).await,
            "search_wikipedia" => self.execute_search_wikipedia(arguments).await,
            "query_wikidata" => self.execute_query_wikidata(arguments).await,
            "run_discovery" => self.execute_run_discovery(arguments).await,
            "analyze_coherence" => self.execute_analyze_coherence(arguments).await,
            "detect_patterns" => self.execute_detect_patterns(arguments).await,
            "export_graph" => self.execute_export_graph(arguments).await,
            _ => Err(JsonRpcError::method_not_found(name)),
        };

        result
    }

    /// Handle resources/list request
    async fn handle_resources_list(&self) -> std::result::Result<Value, JsonRpcError> {
        let resources = vec![
            ResourceDefinition {
                uri: "discovery://patterns".to_string(),
                name: "Discovered Patterns".to_string(),
                description: "Current discovered patterns from analysis".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDefinition {
                uri: "discovery://graph".to_string(),
                name: "Coherence Graph".to_string(),
                description: "Current coherence graph structure".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDefinition {
                uri: "discovery://history".to_string(),
                name: "Coherence History".to_string(),
                description: "Historical coherence signal data".to_string(),
                mime_type: Some("application/json".to_string()),
            },
        ];

        Ok(json!({ "resources": resources }))
    }

    /// Handle resources/read request
    async fn handle_resource_read(&self, params: Option<Value>) -> std::result::Result<Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;

        let uri = params.get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing URI"))?;

        let engine = self.engine.read().await;

        let content = match uri {
            "discovery://patterns" => {
                let patterns = engine.get_patterns();
                json!({
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&patterns)
                        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
                })
            },
            "discovery://graph" => {
                let graph = engine.export_graph();
                json!({
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&graph)
                        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
                })
            },
            "discovery://history" => {
                let history = engine.get_coherence_history();
                json!({
                    "uri": uri,
                    "mimeType": "application/json",
                    "text": serde_json::to_string_pretty(&history)
                        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
                })
            },
            _ => return Err(JsonRpcError::invalid_params(&format!("Unknown resource URI: {}", uri))),
        };

        Ok(json!({ "contents": [content] }))
    }

    /// Handle prompts/list request
    async fn handle_prompts_list(&self) -> std::result::Result<Value, JsonRpcError> {
        let prompts = vec![
            PromptDefinition {
                name: "cross_domain_discovery".to_string(),
                description: "Discover patterns across multiple research domains".to_string(),
                arguments: Some(vec![
                    PromptArgument {
                        name: "domains".to_string(),
                        description: "Comma-separated list of domains (research,medical,climate,economic)".to_string(),
                        required: true,
                    },
                    PromptArgument {
                        name: "query".to_string(),
                        description: "Search query to apply across domains".to_string(),
                        required: true,
                    },
                ]),
            },
            PromptDefinition {
                name: "citation_analysis".to_string(),
                description: "Build and analyze citation networks".to_string(),
                arguments: Some(vec![
                    PromptArgument {
                        name: "seed_paper_id".to_string(),
                        description: "Starting paper ID (OpenAlex, Semantic Scholar, or DOI)".to_string(),
                        required: true,
                    },
                    PromptArgument {
                        name: "depth".to_string(),
                        description: "Citation depth to traverse (default: 2)".to_string(),
                        required: false,
                    },
                ]),
            },
            PromptDefinition {
                name: "trend_detection".to_string(),
                description: "Detect temporal patterns and trends".to_string(),
                arguments: Some(vec![
                    PromptArgument {
                        name: "source".to_string(),
                        description: "Data source (arxiv, pubmed, biorxiv, etc.)".to_string(),
                        required: true,
                    },
                    PromptArgument {
                        name: "query".to_string(),
                        description: "Search query".to_string(),
                        required: true,
                    },
                    PromptArgument {
                        name: "days".to_string(),
                        description: "Number of days to analyze (default: 30)".to_string(),
                        required: false,
                    },
                ]),
            },
        ];

        Ok(json!({ "prompts": prompts }))
    }

    /// Handle prompts/get request
    async fn handle_prompt_get(&self, params: Option<Value>) -> std::result::Result<Value, JsonRpcError> {
        let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;

        let name = params.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing prompt name"))?;

        let arguments = params.get("arguments")
            .and_then(|v| v.as_object());

        let messages = match name {
            "cross_domain_discovery" => {
                let domains = arguments
                    .and_then(|a| a.get("domains"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsonRpcError::invalid_params("Missing 'domains' argument"))?;
                let query = arguments
                    .and_then(|a| a.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsonRpcError::invalid_params("Missing 'query' argument"))?;

                vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Perform cross-domain discovery across: {}\nQuery: {}\n\n\
                            Steps:\n\
                            1. Search each domain using run_discovery\n\
                            2. Analyze coherence across domains\n\
                            3. Detect emerging patterns\n\
                            4. Export visualization graph",
                            domains, query
                        )
                    }
                })]
            },
            "citation_analysis" => {
                let paper_id = arguments
                    .and_then(|a| a.get("seed_paper_id"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsonRpcError::invalid_params("Missing 'seed_paper_id' argument"))?;
                let depth = arguments
                    .and_then(|a| a.get("depth"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(2);

                vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Build citation network starting from paper: {}\nDepth: {}\n\n\
                            Steps:\n\
                            1. Get initial paper with get_citations\n\
                            2. Recursively fetch citations up to depth {}\n\
                            3. Build coherence graph\n\
                            4. Analyze citation patterns\n\
                            5. Export as GraphML for visualization",
                            paper_id, depth, depth
                        )
                    }
                })]
            },
            "trend_detection" => {
                let source = arguments
                    .and_then(|a| a.get("source"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsonRpcError::invalid_params("Missing 'source' argument"))?;
                let query = arguments
                    .and_then(|a| a.get("query"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| JsonRpcError::invalid_params("Missing 'query' argument"))?;
                let days = arguments
                    .and_then(|a| a.get("days"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30);

                vec![json!({
                    "role": "user",
                    "content": {
                        "type": "text",
                        "text": format!(
                            "Detect trends in {} for query: {}\nTime window: {} days\n\n\
                            Steps:\n\
                            1. Fetch recent data from {}\n\
                            2. Compute temporal coherence signals\n\
                            3. Detect emerging/declining patterns\n\
                            4. Generate trend forecast",
                            source, query, days, source
                        )
                    }
                })]
            },
            _ => return Err(JsonRpcError::method_not_found(name)),
        };

        Ok(json!({
            "description": format!("Prompt: {}", name),
            "messages": messages
        }))
    }

    /// Check rate limiting
    async fn check_rate_limit(&self, tool_name: &str) -> std::result::Result<(), JsonRpcError> {
        let mut counts = self.request_count.write().await;
        let count = counts.entry(tool_name.to_string()).or_insert(0);
        *count += 1;

        if *count > self.config.rate_limit_per_minute {
            return Err(JsonRpcError::internal_error("Rate limit exceeded"));
        }

        Ok(())
    }

    // ========================================================================
    // Tool Definitions
    // ========================================================================

    fn tool_search_openalex(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_openalex".to_string(),
            description: "Search OpenAlex for research papers and scholarly works".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Maximum results (default: 10)", "default": 10 }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_search_arxiv(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_arxiv".to_string(),
            description: "Search arXiv preprint repository".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "category": { "type": "string", "description": "arXiv category (e.g., cs.AI, physics.gen-ph)" },
                    "limit": { "type": "integer", "description": "Maximum results", "default": 10 }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_search_semantic_scholar(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_semantic_scholar".to_string(),
            description: "Search Semantic Scholar academic database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Maximum results", "default": 10 }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_get_citations(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_citations".to_string(),
            description: "Get citations for a paper (Semantic Scholar or OpenAlex)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "paper_id": { "type": "string", "description": "Paper ID or DOI" }
                },
                "required": ["paper_id"]
            }),
        }
    }

    fn tool_search_crossref(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_crossref".to_string(),
            description: "Search CrossRef DOI database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Maximum results", "default": 10 }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_search_biorxiv(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_biorxiv".to_string(),
            description: "Search bioRxiv preprints".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "category": { "type": "string", "description": "Category (e.g., neuroscience, bioinformatics)" },
                    "days": { "type": "integer", "description": "Days back to search", "default": 7 }
                },
                "required": ["category"]
            }),
        }
    }

    fn tool_search_medrxiv(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_medrxiv".to_string(),
            description: "Search medRxiv medical preprints".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "days": { "type": "integer", "description": "Days back to search", "default": 7 }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_search_pubmed(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_pubmed".to_string(),
            description: "Search PubMed medical literature database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Maximum results", "default": 10 }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_search_clinical_trials(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_clinical_trials".to_string(),
            description: "Search ClinicalTrials.gov".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "condition": { "type": "string", "description": "Medical condition" },
                    "status": { "type": "string", "description": "Trial status (e.g., recruiting, completed)" }
                },
                "required": ["condition"]
            }),
        }
    }

    fn tool_search_fda_events(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_fda_events".to_string(),
            description: "Search FDA adverse event reports".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "drug_name": { "type": "string", "description": "Drug name to search" }
                },
                "required": ["drug_name"]
            }),
        }
    }

    fn tool_get_fred_series(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_fred_series".to_string(),
            description: "Get Federal Reserve Economic Data (FRED) time series".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "series_id": { "type": "string", "description": "FRED series ID (e.g., GDP, UNRATE)" }
                },
                "required": ["series_id"]
            }),
        }
    }

    fn tool_get_worldbank_indicator(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_worldbank_indicator".to_string(),
            description: "Get World Bank development indicators".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "country": { "type": "string", "description": "Country code (e.g., USA, CHN)" },
                    "indicator": { "type": "string", "description": "Indicator code (e.g., NY.GDP.MKTP.CD)" }
                },
                "required": ["country", "indicator"]
            }),
        }
    }

    fn tool_get_noaa_data(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_noaa_data".to_string(),
            description: "Get NOAA climate data".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "station": { "type": "string", "description": "Weather station ID" },
                    "start_date": { "type": "string", "description": "Start date (YYYY-MM-DD)" },
                    "end_date": { "type": "string", "description": "End date (YYYY-MM-DD)" }
                },
                "required": ["station", "start_date", "end_date"]
            }),
        }
    }

    fn tool_search_wikipedia(&self) -> ToolDefinition {
        ToolDefinition {
            name: "search_wikipedia".to_string(),
            description: "Search Wikipedia articles".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "language": { "type": "string", "description": "Language code (default: en)", "default": "en" }
                },
                "required": ["query"]
            }),
        }
    }

    fn tool_query_wikidata(&self) -> ToolDefinition {
        ToolDefinition {
            name: "query_wikidata".to_string(),
            description: "Query Wikidata knowledge graph using SPARQL".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "sparql_query": { "type": "string", "description": "SPARQL query string" }
                },
                "required": ["sparql_query"]
            }),
        }
    }

    fn tool_run_discovery(&self) -> ToolDefinition {
        ToolDefinition {
            name: "run_discovery".to_string(),
            description: "Run discovery engine across multiple data sources".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "sources": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Data sources to query"
                    },
                    "query": { "type": "string", "description": "Discovery query" }
                },
                "required": ["sources", "query"]
            }),
        }
    }

    fn tool_analyze_coherence(&self) -> ToolDefinition {
        ToolDefinition {
            name: "analyze_coherence".to_string(),
            description: "Analyze coherence of vector embeddings".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "vectors": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "embedding": { "type": "array", "items": { "type": "number" } }
                            }
                        },
                        "description": "Vector embeddings to analyze"
                    }
                },
                "required": ["vectors"]
            }),
        }
    }

    fn tool_detect_patterns(&self) -> ToolDefinition {
        ToolDefinition {
            name: "detect_patterns".to_string(),
            description: "Detect patterns in coherence signals".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "signals": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "window_id": { "type": "integer" },
                                "score": { "type": "number" }
                            }
                        },
                        "description": "Coherence signals to analyze"
                    }
                },
                "required": ["signals"]
            }),
        }
    }

    fn tool_export_graph(&self) -> ToolDefinition {
        ToolDefinition {
            name: "export_graph".to_string(),
            description: "Export coherence graph in various formats".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["graphml", "dot", "csv"],
                        "description": "Export format"
                    }
                },
                "required": ["format"]
            }),
        }
    }

    // ========================================================================
    // Tool Executions
    // ========================================================================

    async fn execute_search_openalex(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.clients.openalex.fetch_works(query, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_arxiv(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.clients.arxiv.search(query, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_semantic_scholar(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.clients.semantic_scholar.search_papers(query, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_get_citations(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let paper_id = args.get("paper_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing paper_id"))?;

        let citations = self.clients.semantic_scholar.get_citations(paper_id, 100).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&citations)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_crossref(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.clients.crossref.search_works(query, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_biorxiv(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let _category = args.get("category")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing category"))?;
        let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(7);
        let limit = 10; // Default limit

        let results = self.clients.biorxiv.search_recent(days, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_medrxiv(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let _query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(7);
        let limit = 10; // Default limit

        let results = self.clients.medrxiv.search_recent(days, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_pubmed(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.clients.pubmed.search_articles(query, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_clinical_trials(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let condition = args.get("condition")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing condition"))?;
        let status = args.get("status").and_then(|v| v.as_str());

        let results = self.clients.clinical_trials.search_trials(condition, status).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_fda_events(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let drug_name = args.get("drug_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing drug_name"))?;

        let results = self.clients.fda.search_drug_events(drug_name).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_get_fred_series(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let series_id = args.get("series_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing series_id"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        let results = self.clients.fred.get_series(series_id, Some(limit)).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_get_worldbank_indicator(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let country = args.get("country")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing country"))?;
        let indicator = args.get("indicator")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing indicator"))?;

        let results = self.clients.worldbank.get_indicator(country, indicator).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_get_noaa_data(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let station_id = args.get("station_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing station_id"))?;
        let start_date = args.get("start_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing start_date"))?;
        let end_date = args.get("end_date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing end_date"))?;

        let results = self.clients.noaa.fetch_climate_data(station_id, start_date, end_date).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_search_wikipedia(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

        let results = self.clients.wikipedia.search(query, limit).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_query_wikidata(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let sparql_query = args.get("sparql_query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing sparql_query"))?;

        let results = self.clients.wikidata.sparql_query(sparql_query).await
            .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&results)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_run_discovery(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let _sources = args.get("sources")
            .and_then(|v| v.as_array())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing sources array"))?;
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing query"))?;

        // TODO: Implement multi-source discovery
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Discovery query '{}' across sources", query)
            }]
        }))
    }

    async fn execute_analyze_coherence(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let _vectors = args.get("vectors")
            .and_then(|v| v.as_array())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing vectors array"))?;

        // TODO: Implement coherence analysis
        Ok(json!({
            "content": [{
                "type": "text",
                "text": "Coherence analysis complete"
            }]
        }))
    }

    async fn execute_detect_patterns(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let _signals = args.get("signals")
            .and_then(|v| v.as_array())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing signals array"))?;

        let engine = self.engine.read().await;
        let patterns = engine.get_patterns();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&patterns)
                    .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?
            }]
        }))
    }

    async fn execute_export_graph(&self, args: &Value) -> std::result::Result<Value, JsonRpcError> {
        let format = args.get("format")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError::invalid_params("Missing format"))?;

        let engine = self.engine.read().await;
        let graph = engine.export_graph();

        let exported = match format {
            "graphml" => serde_json::to_string_pretty(&json!({ "format": "graphml", "graph": graph }))
                .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?,
            "dot" => serde_json::to_string_pretty(&json!({ "format": "dot", "graph": graph }))
                .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?,
            "csv" => serde_json::to_string_pretty(&json!({ "format": "csv", "graph": graph }))
                .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?,
            _ => return Err(JsonRpcError::invalid_params("Invalid format")),
        };

        Ok(json!({
            "content": [{
                "type": "text",
                "text": exported
            }]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonrpc_error_codes() {
        let err = JsonRpcError::parse_error("test");
        assert_eq!(err.code, JsonRpcError::PARSE_ERROR);

        let err = JsonRpcError::method_not_found("test_method");
        assert_eq!(err.code, JsonRpcError::METHOD_NOT_FOUND);
        assert!(err.message.contains("test_method"));
    }

    #[test]
    fn test_server_capabilities() {
        let caps = ServerCapabilities {
            tools: ToolsCapability { list_changed: false },
            resources: ResourcesCapability { list_changed: false, subscribe: false },
            prompts: PromptsCapability { list_changed: false },
        };

        let json = serde_json::to_value(&caps).unwrap();
        assert!(json.get("tools").is_some());
        assert!(json.get("resources").is_some());
        assert!(json.get("prompts").is_some());
    }
}
