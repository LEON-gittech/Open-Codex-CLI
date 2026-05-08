use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub const MEMORY_STAGE_UPDATE_TOOL_NAME: &str = "memory_stage_update";

pub fn create_memory_stage_update_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "content".to_string(),
            JsonSchema::string(Some(
                "Required. The memory content to make available for the rest of this session and stage for durable consolidation."
                    .to_string(),
            )),
        ),
        (
            "reason".to_string(),
            JsonSchema::string(Some(
                "Optional short reason for staging this memory update.".to_string(),
            )),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: MEMORY_STAGE_UPDATE_TOOL_NAME.to_string(),
        description: "Stage an active memory update. The update becomes visible in this session and is written as an ad-hoc note for later durable memory consolidation; it does not directly edit canonical memory files."
            .to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            /*required*/ Some(vec!["content".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    })
}
