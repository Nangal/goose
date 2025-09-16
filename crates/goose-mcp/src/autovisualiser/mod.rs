use base64::{engine::general_purpose::STANDARD, Engine as _};
use etcetera::{choose_app_strategy, AppStrategy};
use indoc::{formatdoc, indoc};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, Content, ErrorCode, ErrorData, Implementation, Resource,
        ResourceContents, Role, ServerCapabilities, ServerInfo,
    },
    schemars::JsonSchema,
    service::RequestContext,
    tool, tool_handler, tool_router, RoleServer, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, future::Future, path::PathBuf, sync::Arc, sync::Mutex};

/// Validates that the data parameter is a proper JSON value and not a string
fn validate_data_param(params: &Value, allow_array: bool) -> Result<Value, ErrorData> {
    let data_value = params.get("data").ok_or_else(|| {
        ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            "Missing 'data' parameter".to_string(),
            None,
        )
    })?;

    if data_value.is_string() {
        return Err(ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            "The 'data' parameter must be a JSON object, not a JSON string. Please provide valid JSON without comments.".to_string(),
            None,
        ));
    }

    if allow_array {
        if !data_value.is_object() && !data_value.is_array() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                "The 'data' parameter must be a JSON object or array.".to_string(),
                None,
            ));
        }
    } else if !data_value.is_object() {
        return Err(ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            "The 'data' parameter must be a JSON object.".to_string(),
            None,
        ));
    }

    Ok(data_value.clone())
}

/// Parameters for render_sankey tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderSankeyParams {
    /// The data for the Sankey diagram
    pub data: serde_json::Value,
}

/// Parameters for render_radar tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderRadarParams {
    /// The data for the radar chart
    pub data: serde_json::Value,
}

/// Parameters for render_donut tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderDonutParams {
    /// The data for the donut chart
    pub data: serde_json::Value,
}

/// Parameters for render_treemap tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderTreemapParams {
    /// The data for the treemap
    pub data: serde_json::Value,
}

/// Parameters for render_chord tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderChordParams {
    /// The data for the chord diagram
    pub data: serde_json::Value,
}

/// Parameters for render_map tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RenderMapParams {
    /// The data for the map visualization
    pub data: serde_json::Value,
}

/// Parameters for show_chart tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ShowChartParams {
    /// The data for the chart
    pub data: serde_json::Value,
}

/// An extension for automatic data visualization and UI generation
#[derive(Clone)]
pub struct AutoVisualiserServer {
    tool_router: ToolRouter<Self>,
    #[allow(dead_code)]
    cache_dir: PathBuf,
    active_resources: Arc<Mutex<HashMap<String, Resource>>>,
}

impl Default for AutoVisualiserServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AutoVisualiserServer {
    fn get_info(&self) -> ServerInfo {
        let instructions = formatdoc! {r#"
            This extension provides tools for automatic data visualization
            Use these tools when you are presenting data to the user which could be complemented by a visual expression
            Choose the most appropriate chart type based on the data you have and can provide
            It is important you match the data format as appropriate with the chart type you have chosen
            The user may specify a type of chart or you can pick one of the most appopriate that you can shape the data to

            ## Available Tools:
            - **render_sankey**: Creates interactive Sankey diagrams from flow data
            - **render_radar**: Creates interactive radar charts for multi-dimensional data comparison
            - **render_donut**: Creates interactive donut/pie charts for categorical data (supports multiple charts)
            - **render_treemap**: Creates interactive treemap visualizations for hierarchical data
            - **render_chord**: Creates interactive chord diagrams for relationship/flow visualization
            - **render_map**: Creates interactive map visualizations with location markers
            - **show_chart**: Creates interactive line, scatter, or bar charts for data visualization
        "#};

        ServerInfo {
            server_info: Implementation {
                name: "goose-autovisualiser".to_string(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            instructions: Some(instructions),
            ..Default::default()
        }
    }
}

#[tool_router(router = tool_router)]
impl AutoVisualiserServer {
    pub fn new() -> Self {
        // choose_app_strategy().cache_dir()
        // - macOS/Linux: ~/.cache/goose/autovisualiser/
        // - Windows:     ~\AppData\Local\Block\goose\cache\autovisualiser\
        let cache_dir = choose_app_strategy(crate::APP_STRATEGY.clone())
            .unwrap()
            .cache_dir()
            .join("autovisualiser");

        // Create cache directory if it doesn't exist
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            tool_router: Self::tool_router(),
            cache_dir,
            active_resources: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// show a Sankey diagram from flow data               
    /// The data must contain:
    /// - nodes: Array of objects with 'name' and optional 'category' properties
    /// - links: Array of objects with 'source', 'target', and 'value' properties
    #[tool(
        name = "render_sankey",
        description = "show a Sankey diagram from flow data. The data must contain nodes and links arrays."
    )]
    pub async fn render_sankey(
        &self,
        params: Parameters<RenderSankeyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/sankey_template.html");
        const D3_MIN: &str = include_str!("templates/assets/d3.min.js");
        const D3_SANKEY: &str = include_str!("templates/assets/d3.sankey.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{D3_MIN}}", D3_MIN)
            .replace("{{D3_SANKY}}", D3_SANKEY) // Note: keeping the typo to match template
            .replace("{{SANKEY_DATA}}", &data_json);

        // Save to /tmp/vis.html for debugging
        let debug_path = std::path::Path::new("/tmp/vis.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/vis.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/vis.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://sankey/diagram".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }

    /// show a radar chart (spider chart) for multi-dimensional data comparison             
    /// The data must contain:
    /// - labels: Array of strings representing the dimensions/axes
    /// - datasets: Array of dataset objects with 'label' and 'data' properties
    #[tool(
        name = "render_radar",
        description = "show a radar chart for multi-dimensional data comparison. The data must contain labels and datasets arrays."
    )]
    pub async fn render_radar(
        &self,
        params: Parameters<RenderRadarParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/radar_template.html");
        const CHART_MIN: &str = include_str!("templates/assets/chart.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{CHART_MIN}}", CHART_MIN)
            .replace("{{RADAR_DATA}}", &data_json);

        // Save to /tmp/radar.html for debugging
        let debug_path = std::path::Path::new("/tmp/radar.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/radar.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/radar.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://radar/chart".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }

    /// show a treemap visualization for hierarchical data with proportional area representation as boxes
    /// The data should be a hierarchical structure with:
    /// - name: Name of the node (required)
    /// - value: Numeric value for leaf nodes (optional for parent nodes)
    /// - children: Array of child nodes (optional)
    /// - category: Category for coloring (optional)
    #[tool(
        name = "render_treemap",
        description = "show a treemap visualization for hierarchical data. The data should have name and optionally value, children, and category fields."
    )]
    pub async fn render_treemap(
        &self,
        params: Parameters<RenderTreemapParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/treemap_template.html");
        const D3_MIN: &str = include_str!("templates/assets/d3.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{D3_MIN}}", D3_MIN)
            .replace("{{TREEMAP_DATA}}", &data_json);

        // Save to /tmp/treemap.html for debugging
        let debug_path = std::path::Path::new("/tmp/treemap.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/treemap.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/treemap.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://treemap/visualization".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }

    /// Show a chord diagram visualization for showing relationships and flows between entities.
    /// The data must contain:
    /// - labels: Array of strings representing the entities
    /// - matrix: 2D array of numbers representing flows (matrix[i][j] = flow from i to j)
    #[tool(
        name = "render_chord",
        description = "Show a chord diagram for relationships and flows between entities. The data must contain labels and matrix arrays."
    )]
    pub async fn render_chord(
        &self,
        params: Parameters<RenderChordParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/chord_template.html");
        const D3_MIN: &str = include_str!("templates/assets/d3.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{D3_MIN}}", D3_MIN)
            .replace("{{CHORD_DATA}}", &data_json);

        // Save to /tmp/chord.html for debugging
        let debug_path = std::path::Path::new("/tmp/chord.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/chord.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/chord.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://chord/diagram".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }

    /// show pie or donut charts for categorical data visualization
    /// Supports single or multiple charts in a grid layout.
    #[tool(
        name = "render_donut",
        description = "show pie or donut charts for categorical data visualization. Supports single or multiple charts in a grid layout."
    )]
    pub async fn render_donut(
        &self,
        params: Parameters<RenderDonutParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/donut_template.html");
        const CHART_MIN: &str = include_str!("templates/assets/chart.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{CHART_MIN}}", CHART_MIN)
            .replace("{{CHARTS_DATA}}", &data_json);

        // Save to /tmp/donut.html for debugging
        let debug_path = std::path::Path::new("/tmp/donut.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/donut.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/donut.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://donut/chart".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }

    /// show an interactive map visualization with location markers using Leaflet.
    #[tool(
        name = "render_map",
        description = "show an interactive map visualization with location markers. The data must contain a markers array."
    )]
    pub async fn render_map(
        &self,
        params: Parameters<RenderMapParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Extract title and subtitle from data if provided
        let title = data
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Interactive Map");
        let subtitle = data
            .get("subtitle")
            .and_then(|v| v.as_str())
            .unwrap_or("Geographic data visualization");

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/map_template.html");
        const LEAFLET_JS: &str = include_str!("templates/assets/leaflet.min.js");
        const LEAFLET_CSS: &str = include_str!("templates/assets/leaflet.min.css");
        const MARKERCLUSTER_JS: &str =
            include_str!("templates/assets/leaflet.markercluster.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{LEAFLET_JS}}", LEAFLET_JS)
            .replace("{{LEAFLET_CSS}}", LEAFLET_CSS)
            .replace("{{MARKERCLUSTER_JS}}", MARKERCLUSTER_JS)
            .replace("{{MAP_DATA}}", &data_json)
            .replace("{{TITLE}}", title)
            .replace("{{SUBTITLE}}", subtitle);

        // Save to /tmp/map.html for debugging
        let debug_path = std::path::Path::new("/tmp/map.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/map.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/map.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://map/visualization".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }

    /// show interactive line, scatter, or bar charts
    /// Required: type ('line', 'scatter', or 'bar'), datasets array
    #[tool(
        name = "show_chart",
        description = "show interactive line, scatter, or bar charts. The data must contain type and datasets fields."
    )]
    pub async fn show_chart(
        &self,
        params: Parameters<ShowChartParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let data = params.0.data;

        // Convert the data to JSON string
        let data_json = serde_json::to_string(&data).map_err(|e| {
            ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                format!("Invalid JSON data: {}", e),
                None,
            )
        })?;

        // Load all resources at compile time using include_str!
        const TEMPLATE: &str = include_str!("templates/chart_template.html");
        const CHART_MIN: &str = include_str!("templates/assets/chart.min.js");

        // Replace all placeholders with actual content
        let html_content = TEMPLATE
            .replace("{{CHART_MIN}}", CHART_MIN)
            .replace("{{CHART_DATA}}", &data_json);

        // Save to /tmp/chart.html for debugging
        let debug_path = std::path::Path::new("/tmp/chart.html");
        if let Err(e) = std::fs::write(debug_path, &html_content) {
            tracing::warn!("Failed to write debug HTML to /tmp/chart.html: {}", e);
        } else {
            tracing::info!("Debug HTML saved to /tmp/chart.html");
        }

        // Use BlobResourceContents with base64 encoding to avoid JSON string escaping issues
        let html_bytes = html_content.as_bytes();
        let base64_encoded = STANDARD.encode(html_bytes);

        let resource_contents = ResourceContents::BlobResourceContents {
            uri: "ui://chart/interactive".to_string(),
            mime_type: Some("text/html".to_string()),
            blob: base64_encoded,
            meta: None,
        };

        Ok(vec![
            Content::resource(resource_contents).with_audience(vec![Role::User])
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::handler::server::wrapper::Parameters;
    use rmcp::model::RawContent;
    use serde_json::json;

    #[test]
    fn test_validate_data_param_rejects_string() {
        // Test that a string value for data is rejected
        let params = json!({
            "data": "{\"labels\": [\"A\", \"B\"], \"matrix\": [[0, 1], [1, 0]]}"
        });

        let result = validate_data_param(&params, false);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err
            .message
            .contains("must be a JSON object, not a JSON string"));
        assert!(err.message.contains("without comments"));
    }

    #[test]
    fn test_validate_data_param_accepts_object() {
        // Test that a proper object is accepted
        let params = json!({
            "data": {
                "labels": ["A", "B"],
                "matrix": [[0, 1], [1, 0]]
            }
        });

        let result = validate_data_param(&params, false);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.is_object());
        assert_eq!(data["labels"][0], "A");
    }

    #[test]
    fn test_validate_data_param_rejects_array_when_not_allowed() {
        // Test that an array is rejected when allow_array is false
        let params = json!({
            "data": [
                {"label": "A", "value": 10},
                {"label": "B", "value": 20}
            ]
        });

        let result = validate_data_param(&params, false);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("must be a JSON object"));
    }

    #[test]
    fn test_validate_data_param_accepts_array_when_allowed() {
        // Test that an array is accepted when allow_array is true
        let params = json!({
            "data": [
                {"label": "A", "value": 10},
                {"label": "B", "value": 20}
            ]
        });

        let result = validate_data_param(&params, true);
        assert!(result.is_ok());

        let data = result.unwrap();
        assert!(data.is_array());
        assert_eq!(data[0]["label"], "A");
    }

    #[test]
    fn test_validate_data_param_missing_data() {
        // Test that missing data parameter is rejected
        let params = json!({
            "other": "value"
        });

        let result = validate_data_param(&params, false);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("Missing 'data' parameter"));
    }

    #[test]
    fn test_validate_data_param_rejects_primitive_values() {
        // Test that primitive values (number, boolean) are rejected
        let params_number = json!({
            "data": 42
        });

        let result = validate_data_param(&params_number, false);
        assert!(result.is_err());

        let params_bool = json!({
            "data": true
        });

        let result = validate_data_param(&params_bool, false);
        assert!(result.is_err());

        let params_null = json!({
            "data": null
        });

        let result = validate_data_param(&params_null, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_data_param_with_json_containing_comments_as_string() {
        // Test that JSON with comments passed as a string is rejected
        let params = json!({
            "data": r#"{
                "labels": ["A", "B"],
                "matrix": [
                    [0, 1],  // This is a comment
                    [1, 0]   /* Another comment */
                ]
            }"#
        });

        let result = validate_data_param(&params, false);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::INVALID_PARAMS);
        assert!(err.message.contains("not a JSON string"));
        assert!(err.message.contains("without comments"));
    }

    #[tokio::test]
    async fn test_render_sankey() {
        let server = AutoVisualiserServer::new();
        let data = json!({
            "nodes": [{"name": "A"}, {"name": "B"}],
            "links": [{"source": "A", "target": "B", "value": 10}]
        });
        
        let params = Parameters(RenderSankeyParams { data });
        let result = server.render_sankey(params).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);

        // Check it's a resource with HTML content
        // Content is Annotated<RawContent>, access underlying RawContent via *
        if let RawContent::Resource(resource) = &*content[0] {
            if let ResourceContents::BlobResourceContents { uri, mime_type, .. } =
                &resource.resource
            {
                assert_eq!(uri, "ui://sankey/diagram");
                assert_eq!(mime_type.as_ref().unwrap(), "text/html");
            } else {
                panic!("Expected BlobResourceContents");
            }
        } else {
            panic!("Expected Resource content");
        }
    }

    #[tokio::test]
    async fn test_render_radar() {
        let server = AutoVisualiserServer::new();
        let data = json!({
            "labels": ["Speed", "Power", "Agility"],
            "datasets": [
                {"label": "Player 1", "data": [80, 90, 85]}
            ]
        });
        
        let params = Parameters(RenderRadarParams { data });
        let result = server.render_radar(params).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);

        // Check it's a resource with HTML content
        // Content is Annotated<RawContent>, access underlying RawContent via *
        if let RawContent::Resource(resource) = &*content[0] {
            if let ResourceContents::BlobResourceContents {
                uri,
                mime_type,
                blob,
                ..
            } = &resource.resource
            {
                assert_eq!(uri, "ui://radar/chart");
                assert_eq!(mime_type.as_ref().unwrap(), "text/html");
                assert!(!blob.is_empty(), "HTML content should not be empty");
            } else {
                panic!("Expected BlobResourceContents");
            }
        } else {
            panic!("Expected Resource content");
        }
    }

    #[tokio::test]
    async fn test_render_donut() {
        let server = AutoVisualiserServer::new();
        let data = json!({
            "labels": ["A", "B", "C"],
            "values": [30, 40, 30]
        });
        
        let params = Parameters(RenderDonutParams { data });
        let result = server.render_donut(params).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);
    }

    #[tokio::test]
    async fn test_render_treemap() {
        let server = AutoVisualiserServer::new();
        let data = json!({
            "name": "root",
            "children": [
                {"name": "A", "value": 100},
                {"name": "B", "value": 200}
            ]
        });
        
        let params = Parameters(RenderTreemapParams { data });
        let result = server.render_treemap(params).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);
    }

    #[tokio::test]
    async fn test_render_chord() {
        let server = AutoVisualiserServer::new();
        let data = json!({
            "labels": ["A", "B", "C"],
            "matrix": [[0, 10, 5], [10, 0, 15], [5, 15, 0]]
        });
        
        let params = Parameters(RenderChordParams { data });
        let result = server.render_chord(params).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);
    }

    #[tokio::test]
    async fn test_render_map() {
        let server = AutoVisualiserServer::new();
        let data = json!({
            "markers": [
                {"lat": 37.7749, "lng": -122.4194, "name": "SF Store", "value": 150000},
                {"lat": 40.7128, "lng": -74.0060, "name": "NYC Store", "value": 200000}
            ]
        });
        
        let params = Parameters(RenderMapParams { data });
        let result = server.render_map(params).await;
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);
    }

    #[tokio::test]
    async fn test_show_chart() {
        let server = AutoVisualiserServer::new();
        // show_chart expects data to be an object, not an array
        let data = json!({
            "type": "line",
            "datasets": [
                {
                    "label": "Test Data",
                    "data": [
                        {"x": 1, "y": 2},
                        {"x": 2, "y": 4}
                    ]
                }
            ]
        });
        
        let params = Parameters(ShowChartParams { data });
        let result = server.show_chart(params).await;
        if let Err(e) = &result {
            eprintln!("Error in test_show_chart: {:?}", e);
        }
        assert!(result.is_ok());
        let content = result.unwrap();
        assert_eq!(content.len(), 1);

        // Check the audience is set to User
        assert!(content[0].audience().is_some());
        assert_eq!(content[0].audience().unwrap(), &vec![Role::User]);
    }
}
