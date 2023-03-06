#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    attr, from_binary, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, Uint128,
    WasmMsg, WasmQuery, QueryRequest,Order, Addr, Storage
};
use cw2::{get_contract_version, set_contract_version};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg, Cw20QueryMsg, TokenInfoResponse, Denom};
use cw_utils::{maybe_addr};
use cw_storage_plus::Bound;
use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StakerListResponse, StakerInfo, ApyRecord, ReceiveMsg, ApyType
};
use crate::state::{
    Config, CONFIG, STAKERS
};


use crate::util;
use crate::constants;

// Version info, for migration info
const CONTRACT_NAME: &str = "incentive";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MULTIPLE:u128 = 100u128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;


    let config = Config {
        owner: info.sender.clone(),
        stake_token_address: msg.stake_token_address,
        reward_token_denom: msg.reward_token_denom,
        apys: msg.apys,
        reward_interval: msg.reward_interval,
        enabled: true
    };
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateOwner { owner } => execute_update_owner(deps, info, owner),
        ExecuteMsg::UpdateEnabled { enabled } => execute_update_enabled(deps, info, enabled),
        ExecuteMsg::UpdateConstants { apys, reward_interval} => execute_update_constants(deps, info, apys, reward_interval),
        ExecuteMsg::Receive(msg) => execute_receive(deps, env, info, msg),
        ExecuteMsg::WithdrawReward {amount} => execute_withdraw_reward(deps, env, info, amount),
        ExecuteMsg::WithdrawStake {amount} => execute_withdraw_stake(deps, env, info, amount),
        ExecuteMsg::ClaimReward {index} => execute_claim_reward(deps, env, info, index),
        ExecuteMsg::Unstake {index} => execute_unstake(deps, env, info, index)
    }  
}


pub fn execute_receive(
    deps: DepsMut, 
    env: Env,
    info: MessageInfo, 
    wrapper: Cw20ReceiveMsg
) -> Result<Response, ContractError> {
    
    check_enabled(&deps, &info)?;
    let mut cfg = CONFIG.load(deps.storage)?;
    
    if wrapper.amount == Uint128::zero() {
        return Err(ContractError::InvalidInput {});
    }
    let user_addr = &deps.api.addr_validate(&wrapper.sender)?;

    if info.sender.clone() != cfg.stake_token_address {
        return Err(ContractError::UnacceptableToken {  });
    }

    let msg: ReceiveMsg = from_binary(&wrapper.msg)?;
    match msg {
        ReceiveMsg::Stake {apy_index} => {
            let mut list = STAKERS.load(deps.storage, user_addr.clone()).unwrap_or(vec![]);
            list.push(StakerInfo {
                apy_index,
                address: user_addr.clone(),
                amount: wrapper.amount,
                reward: Uint128::zero(),
                last_time: env.block.time.seconds()
            });
            STAKERS.save(deps.storage, user_addr.clone(), &list)?;

            return Ok(Response::new()
            .add_attributes(vec![
                attr("action", "stake"),
                attr("address", user_addr.clone()),
                attr("amount", wrapper.amount),
            ]));
        } 
    }
    
}

pub fn execute_claim_reward(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    index: u64
) -> Result<Response, ContractError> {

    check_enabled(&deps, &info)?;
    let mut cfg = CONFIG.load(deps.storage)?;


    let mut list = STAKERS.load(deps.storage, info.sender.clone())?;
    
    if (list.len() as u64) < index + 1 {
        return Err(ContractError::StakingRecordIndexOverflow {  });
    }

    let mut record = list[index as usize].clone();

    let mut reward = Uint128::from(0u128);
    
    // Start
    let staked_time = env.block.time.seconds() - record.last_time;
    let mut reward_tot = Uint128::zero();

    if staked_time >= constants::TWO_YEAR_SECONDS { // 100% for over 2 years
        reward_tot = Uint128::from(record.amount);
        
    } else if staked_time >= constants::ONE_YEAR_SECONDS { // 40% for over 1 years
        reward_tot =record.amount.checked_mul(Uint128::from(constants::ONE_YEAR_APY)).unwrap().checked_div(Uint128::from(MULTIPLE)).unwrap() ; 

    } else if staked_time >= constants::SIX_MONTH_SECONDS { // 20% for over 6 months
        reward_tot = record.amount.checked_mul(Uint128::from(constants::SIX_MONTH_APY)).unwrap().checked_div(Uint128::from(MULTIPLE)).unwrap() ; 

    } else if staked_time >= constants::ONE_MONTH_SECONDS { // 10% for over 30 days
        reward_tot =record.amount.checked_mul(Uint128::from(constants::ONE_MONTH_APY)).unwrap().checked_div(Uint128::from(MULTIPLE)).unwrap() ; 

    } else {
        reward_tot = Uint128::zero();
    }
    reward = reward_tot.checked_mul(Uint128::from(7u64)).unwrap().checked_div(Uint128::from(365u64)).unwrap();

    record.last_time = env.block.time.seconds();
    // End

    list[index as usize] = record;
    STAKERS.save(deps.storage, info.sender.clone(), &list)?;

    let tot_reward = util::get_token_amount(deps.querier, Denom::Native(cfg.reward_token_denom.clone()), env.contract.address.clone())?;

    if tot_reward < reward {
        return Err(ContractError::NotEnoughReward {});
    }

    let msg = util::transfer_token_message(Denom::Native(cfg.reward_token_denom.clone()), reward, info.sender.clone())?;

    return Ok(Response::new()
        .add_message(msg)
        .add_attributes(vec![
            attr("action", "claim_reward"),
            attr("address", info.sender.clone()),
            attr("reward_amount", Uint128::from(reward)),
        ]));
}


pub fn execute_unstake(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    index: u64
) -> Result<Response, ContractError> {

    check_enabled(&deps, &info)?;
    let mut cfg = CONFIG.load(deps.storage)?;

    let mut list = STAKERS.load(deps.storage, info.sender.clone())?;
    
    if (list.len() as u64) < index + 1 {
        return Err(ContractError::StakingRecordIndexOverflow {  });
    }

    // check if user can unstake this record
    // env.block.time.seconds(), record.stake_time
    
    let mut record = list[index as usize].clone();
    record.last_time = env.block.time.seconds();
    let staked = record.amount;
    list.remove(index as usize);
    STAKERS.save(deps.storage, info.sender.clone(), &list)?;

    let tot_staked = util::get_token_amount(deps.querier, Denom::Cw20(cfg.stake_token_address.clone()), env.contract.address.clone())?;

    if tot_staked < staked {
        return Err(ContractError::NotEnoughStake {});
    }

    let msg = util::transfer_token_message(Denom::Cw20(cfg.stake_token_address.clone()), staked, info.sender.clone())?;

    return Ok(Response::new()
        .add_message(msg)
        .add_attributes(vec![
            attr("action", "unstake"),
            attr("address", info.sender.clone()),
            attr("staked_amount", Uint128::from(staked)),
        ]));
}

pub fn check_owner(
    deps: &DepsMut,
    info: &MessageInfo
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {})
    }
    Ok(Response::new().add_attribute("action", "check_owner"))
}
pub fn check_enabled(
    deps: &DepsMut,
    info: &MessageInfo
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if !cfg.enabled {
        return Err(ContractError::Disabled {})
    }
    Ok(Response::new().add_attribute("action", "check_enabled"))
}

pub fn execute_update_owner(
    deps: DepsMut,
    info: MessageInfo,
    owner: Addr,
) -> Result<Response, ContractError> {
    // authorize owner
    check_owner(&deps, &info)?;
    
    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.owner = owner;
        Ok(exists)
    })?;
    Ok(Response::new().add_attribute("action", "update_owner"))
}


pub fn execute_update_enabled(
    deps: DepsMut,
    info: MessageInfo,
    enabled: bool
) -> Result<Response, ContractError> {
    // authorize owner
    check_owner(&deps, &info)?;
    
    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.enabled = enabled;
        Ok(exists)
    })?;
    Ok(Response::new().add_attribute("action", "update_enabled"))
}

pub fn execute_update_constants(
    deps: DepsMut,
    info: MessageInfo,
    apys: Vec<ApyRecord>,
    reward_interval: u64
) -> Result<Response, ContractError> {
    // authorize owner
    check_owner(&deps, &info)?;
    
    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.apys = apys;
        exists.reward_interval = reward_interval;
        Ok(exists)
    })?;

    Ok(Response::new().add_attribute("action", "update_constants"))
}


pub fn execute_withdraw_reward(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint128) -> Result<Response, ContractError> {
    
    check_owner(&deps, &info)?;
     
    let mut cfg = CONFIG.load(deps.storage)?;

    let tot = util::get_token_amount(deps.querier, Denom::Native(cfg.reward_token_denom.clone()), env.contract.address.clone())?;

    if tot < amount {
        return Err(ContractError::NotEnoughReward {  });
    }

    let msg = util::transfer_token_message(Denom::Native(cfg.reward_token_denom.clone()), amount, info.sender.clone())?;

    return Ok(Response::new()
        .add_message(msg)
        .add_attributes(vec![
            attr("action", "withdraw_reward"),
            attr("address", info.sender.clone()),
            attr("amount", amount),
        ]));
}

pub fn execute_withdraw_stake(deps: DepsMut, env: Env, info: MessageInfo, amount: Uint128) -> Result<Response, ContractError> {
    
    check_owner(&deps, &info)?;
     
    let mut cfg = CONFIG.load(deps.storage)?;

    let tot = util::get_token_amount(deps.querier, Denom::Cw20(cfg.stake_token_address.clone()), env.contract.address.clone())?;

    if tot < amount {
        return Err(ContractError::NotEnoughStake {  });
    }

    let msg = util::transfer_token_message(Denom::Cw20(cfg.stake_token_address.clone()), amount, info.sender.clone())?;

    return Ok(Response::new()
        .add_message(msg)
        .add_attributes(vec![
            attr("action", "withdraw_stake"),
            attr("address", info.sender.clone()),
            attr("amount", amount),
        ]));
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} 
            => to_binary(&query_config(deps)?),
        QueryMsg::Staker {address} 
            => to_binary(&query_staker(deps, address)?),
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner,
        reward_token_denom: cfg.reward_token_denom.into(),
        stake_token_address: cfg.stake_token_address.into(),
        reward_interval: cfg.reward_interval,
        apys: cfg.apys,
        enabled: cfg.enabled
    })
}

// settings for pagination
const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

fn query_staker(deps: Deps, address: Addr) -> StdResult<Vec<StakerInfo>> {
    
    let list = STAKERS.load(deps.storage, address.clone()).unwrap();
    Ok(list)
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    let version = get_contract_version(deps.storage)?;
    if version.contract != CONTRACT_NAME {
        return Err(ContractError::CannotMigrate {
            previous_contract: version.contract,
        });
    }
    Ok(Response::default())
}

