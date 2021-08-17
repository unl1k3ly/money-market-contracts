use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RewardContractExecuteMsg {
    /// Request bAsset reward withdrawal
    ClaimRewards { recipient: Option<String> },
}
