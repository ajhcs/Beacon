use crate::types::BeaconIR;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn parse_ir(json: &str) -> Result<BeaconIR, ParseError> {
    Ok(serde_json::from_str(json)?)
}
