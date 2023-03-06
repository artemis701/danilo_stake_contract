use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw20::{Cw20ReceiveMsg, Denom};
use cosmwasm_std::{Uint128, Addr};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct InstantiateMsg {
    pub stake_token_address: Addr,
    pub reward_token_denom: String,
    pub apys: Vec<ApyRecord>,
    pub reward_interval: u64
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StakerInfo {
    pub apy_index: u64,
    pub address: Addr,
    pub amount: Uint128,
    pub reward: Uint128,
    pub last_time: u64
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ApyRecord {
    pub duration: u64,
    pub rate: u64
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateOwner {
        owner: Addr
    },
    UpdateEnabled {
        enabled: bool
    },
    UpdateConstants {
        apys: Vec<ApyRecord>,
        reward_interval: u64
    },
    Receive(Cw20ReceiveMsg),
    WithdrawReward { amount: Uint128 },
    WithdrawStake { amount: Uint128 },
    ClaimReward { index: u64},
    Unstake {
        index: u64
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiveMsg {
    Stake { apy_index: u64},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    Staker {
        address: Addr
    },
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ConfigResponse {
    pub owner: Addr,
    pub stake_token_address: Addr,
    pub reward_token_denom: String,
    pub apys: Vec<ApyRecord>,
    pub reward_interval: u64,
    pub enabled: bool

}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct StakerListResponse {
    pub stakers: Vec<StakerInfo>,
}


#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub enum ApyType {
    Two_Year_Stake,
    One_Year_Stake,
    Six_Month_Stake,
    Thirty_Days_Stake
}