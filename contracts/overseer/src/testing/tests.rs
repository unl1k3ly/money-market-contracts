use cosmwasm_std::{
    from_binary, log, to_binary, BankMsg, Coin, CosmosMsg, Decimal, HumanAddr, StdError, Uint128,
    WasmMsg,
};

use crate::contract::{handle, init};
use crate::msg::{
    AllCollateralsResponse, BorrowLimitResponse, CollateralsResponse, ConfigResponse, HandleMsg,
    InitMsg, QueryMsg, WhitelistResponse, WhitelistResponseElem,
};
use crate::querier::query;
use crate::state::EpochState;
use crate::testing::mock_querier::mock_dependencies;

use cosmwasm_std::testing::{mock_env, MOCK_CONTRACT_ADDR};
use moneymarket::{deduct_tax, CustodyHandleMsg};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(20, &[]);

    let msg = InitMsg {
        owner_addr: HumanAddr::from("owner"),
        oracle_contract: HumanAddr::from("oracle"),
        market_contract: HumanAddr::from("market"),
        base_denom: "uusd".to_string(),
        distribution_threshold: Decimal::permille(3),
        target_deposit_rate: Decimal::permille(5),
        buffer_distribution_rate: Decimal::percent(20),
    };

    let env = mock_env("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    let query_res = query(&deps, QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&query_res).unwrap();
    assert_eq!(HumanAddr::from("owner"), config_res.owner_addr);
    assert_eq!(HumanAddr::from("oracle"), config_res.oracle_contract);
    assert_eq!(HumanAddr::from("market"), config_res.market_contract);
    assert_eq!("uusd".to_string(), config_res.base_denom);
    assert_eq!(Decimal::permille(3), config_res.distribution_threshold);
    assert_eq!(Decimal::permille(5), config_res.target_deposit_rate);
    assert_eq!(Decimal::percent(20), config_res.buffer_distribution_rate);

    let query_res = query(&deps, QueryMsg::EpochState {}).unwrap();
    let epoch_state: EpochState = from_binary(&query_res).unwrap();
    assert_eq!(Decimal::zero(), epoch_state.deposit_rate);
    assert_eq!(env.block.height, epoch_state.last_executed_height);
    assert_eq!(Uint128::zero(), epoch_state.prev_a_token_supply);
    assert_eq!(Decimal::one(), epoch_state.prev_exchange_rate);
}

#[test]
fn update_config() {
    let mut deps = mock_dependencies(20, &[]);

    let env = mock_env("addr0000", &[]);
    let msg = InitMsg {
        owner_addr: HumanAddr::from("owner"),
        oracle_contract: HumanAddr::from("oracle"),
        market_contract: HumanAddr::from("market"),
        base_denom: "uusd".to_string(),
        distribution_threshold: Decimal::permille(3),
        target_deposit_rate: Decimal::permille(5),
        buffer_distribution_rate: Decimal::percent(20),
    };

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    // update owner
    let env = mock_env("owner", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner_addr: Some(HumanAddr("owner1".to_string())),
        distribution_threshold: None,
        target_deposit_rate: None,
        buffer_distribution_rate: None,
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(HumanAddr::from("owner1"), config_res.owner_addr);

    // update left items
    let env = mock_env("owner1", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner_addr: None,
        distribution_threshold: Some(Decimal::permille(1)),
        target_deposit_rate: Some(Decimal::permille(2)),
        buffer_distribution_rate: Some(Decimal::percent(10)),
    };

    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(0, res.messages.len());

    // it worked, let's query the state
    let res = query(&deps, QueryMsg::Config {}).unwrap();
    let config_res: ConfigResponse = from_binary(&res).unwrap();
    assert_eq!(HumanAddr::from("owner1"), config_res.owner_addr);
    assert_eq!(Decimal::permille(1), config_res.distribution_threshold);
    assert_eq!(Decimal::permille(2), config_res.target_deposit_rate);
    assert_eq!(Decimal::percent(10), config_res.buffer_distribution_rate);

    // Unauthorzied err
    let env = mock_env("owner", &[]);
    let msg = HandleMsg::UpdateConfig {
        owner_addr: None,
        distribution_threshold: None,
        target_deposit_rate: None,
        buffer_distribution_rate: None,
    };

    let res = handle(&mut deps, env, msg);
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("Must return unauthorized error"),
    }
}

#[test]
fn whitelist() {
    let mut deps = mock_dependencies(20, &[]);

    let env = mock_env("addr0000", &[]);
    let msg = InitMsg {
        owner_addr: HumanAddr::from("owner"),
        oracle_contract: HumanAddr::from("oracle"),
        market_contract: HumanAddr::from("market"),
        base_denom: "uusd".to_string(),
        distribution_threshold: Decimal::permille(3),
        target_deposit_rate: Decimal::permille(5),
        buffer_distribution_rate: Decimal::percent(20),
    };

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env, msg).unwrap();

    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("bluna"),
        custody_contract: HumanAddr::from("custody"),
        ltv: Decimal::percent(60),
    };

    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg.clone());
    match res {
        Err(StdError::Unauthorized { .. }) => {}
        _ => panic!("DO NOT ENTER HERE"),
    };

    let env = mock_env("owner", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.log,
        vec![
            log("action", "register_whitelist"),
            log("collateral_token", "bluna"),
            log("custody_contract", "custody"),
            log("LTV", "0.6")
        ]
    );

    let res = query(
        &deps,
        QueryMsg::Whitelist {
            collateral_token: Some(HumanAddr::from("bluna")),
            start_after: None,
            limit: None,
        },
    )
    .unwrap();
    let whitelist_res: WhitelistResponse = from_binary(&res).unwrap();
    assert_eq!(
        whitelist_res,
        WhitelistResponse {
            elems: vec![WhitelistResponseElem {
                collateral_token: HumanAddr::from("bluna"),
                custody_contract: HumanAddr::from("custody"),
                ltv: Decimal::percent(60)
            }]
        }
    );
}

#[test]
fn execute_epoch_operations() {
    let mut deps = mock_dependencies(
        20,
        &[Coin {
            denom: "uusd".to_string(),
            amount: Uint128::from(10000000000u128),
        }],
    );

    let mut env = mock_env("owner", &[]);
    let msg = InitMsg {
        owner_addr: HumanAddr::from("owner"),
        oracle_contract: HumanAddr::from("oracle"),
        market_contract: HumanAddr::from("market"),
        base_denom: "uusd".to_string(),
        distribution_threshold: Decimal::from_ratio(1u128, 1000000u128),
        target_deposit_rate: Decimal::permille(5),
        buffer_distribution_rate: Decimal::percent(20),
    };

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // store whitelist elems
    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("bluna"),
        custody_contract: HumanAddr::from("custody_bluna"),
        ltv: Decimal::percent(60),
    };

    let _res = handle(&mut deps, env.clone(), msg);

    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("batom"),
        custody_contract: HumanAddr::from("custody_batom"),
        ltv: Decimal::percent(60),
    };

    let _res = handle(&mut deps, env.clone(), msg);

    let msg = HandleMsg::ExecuteEpochOperations {};
    let res = handle(&mut deps, env.clone(), msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "Epoch period is not passed"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    env.block.height += 86400u64;

    // If deposit_rate is bigger than distribution_threshold
    deps.querier.with_epoch_state(&[(
        &HumanAddr::from("market"),
        &(Uint128::from(1000000u128), Decimal::percent(120)),
    )]);

    // (120 / 100 - 1) / 86400
    // deposit rate = 0.000002314814814814
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("custody_batom"),
                send: vec![],
                msg: to_binary(&CustodyHandleMsg::DistributeRewards {}).unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("custody_bluna"),
                send: vec![],
                msg: to_binary(&CustodyHandleMsg::DistributeRewards {}).unwrap(),
            }),
        ]
    );

    assert_eq!(
        res.log,
        vec![
            log("action", "epoch_operations"),
            log("distributed_interest", "0"),
            log("deposit_rate", "0.000002314814814814"),
            log("exchange_rate", "1.2"),
            log("a_token_supply", "1000000"),
        ]
    );

    // If deposit rate is bigger than threshold
    deps.querier.with_epoch_state(&[(
        &HumanAddr::from("market"),
        &(Uint128::from(1000000u128), Decimal::percent(125)),
    )]);

    deps.querier.with_tax(
        Decimal::percent(1),
        &[(&"uusd".to_string(), &Uint128::from(1000000u128))],
    );

    env.block.height += 86400u64;

    // (125 / 120 - 1) / 86400
    // deposit rate = 0.000000482253078703
    let res = handle(&mut deps, env.clone(), msg.clone()).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Bank(BankMsg::Send {
                from_address: HumanAddr::from(MOCK_CONTRACT_ADDR),
                to_address: HumanAddr::from("market"),
                amount: vec![deduct_tax(
                    &deps,
                    Coin {
                        denom: "uusd".to_string(),
                        amount: Uint128::from(53706u128),
                    }
                )
                .unwrap()]
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("custody_batom"),
                send: vec![],
                msg: to_binary(&CustodyHandleMsg::DistributeRewards {}).unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("custody_bluna"),
                send: vec![],
                msg: to_binary(&CustodyHandleMsg::DistributeRewards {}).unwrap(),
            }),
        ]
    );

    assert_eq!(
        res.log,
        vec![
            log("action", "epoch_operations"),
            log("distributed_interest", "53706"),
            log("deposit_rate", "0.000000482253078703"),
            log("exchange_rate", "1.25"),
            log("a_token_supply", "1000000"),
        ]
    );
}

#[test]
fn lock_collateral() {
    let mut deps = mock_dependencies(20, &[]);

    let env = mock_env("owner", &[]);
    let msg = InitMsg {
        owner_addr: HumanAddr::from("owner"),
        oracle_contract: HumanAddr::from("oracle"),
        market_contract: HumanAddr::from("market"),
        base_denom: "uusd".to_string(),
        distribution_threshold: Decimal::permille(3),
        target_deposit_rate: Decimal::permille(5),
        buffer_distribution_rate: Decimal::percent(20),
    };

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // store whitelist elems
    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("bluna"),
        custody_contract: HumanAddr::from("custody_bluna"),
        ltv: Decimal::percent(60),
    };

    let _res = handle(&mut deps, env.clone(), msg);

    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("batom"),
        custody_contract: HumanAddr::from("custody_batom"),
        ltv: Decimal::percent(60),
    };

    let _res = handle(&mut deps, env.clone(), msg);

    let msg = HandleMsg::LockCollateral {
        collaterals: vec![
            (HumanAddr::from("bluna"), Uint128::from(1000000u128)),
            (HumanAddr::from("batom"), Uint128::from(10000000u128)),
        ],
    };
    let env = mock_env("addr0000", &[]);
    let res = handle(&mut deps, env, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("custody_bluna"),
                send: vec![],
                msg: to_binary(&CustodyHandleMsg::LockCollateral {
                    borrower: HumanAddr::from("addr0000"),
                    amount: Uint128::from(1000000u128),
                })
                .unwrap(),
            }),
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: HumanAddr::from("custody_batom"),
                send: vec![],
                msg: to_binary(&CustodyHandleMsg::LockCollateral {
                    borrower: HumanAddr::from("addr0000"),
                    amount: Uint128::from(10000000u128),
                })
                .unwrap(),
            })
        ]
    );

    assert_eq!(
        res.log,
        vec![
            log("action", "lock_collateral"),
            log("borrower", "addr0000"),
            log("collaterals", "1000000bluna,10000000batom"),
        ]
    );

    let res = query(
        &deps,
        QueryMsg::Collaterals {
            borrower: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let collaterals_res: CollateralsResponse = from_binary(&res).unwrap();
    assert_eq!(
        collaterals_res,
        CollateralsResponse {
            borrower: HumanAddr::from("addr0000"),
            collaterals: vec![
                (HumanAddr::from("batom"), Uint128::from(10000000u128)),
                (HumanAddr::from("bluna"), Uint128::from(1000000u128)),
            ]
        }
    );

    let res = query(
        &deps,
        QueryMsg::AllCollaterals {
            start_after: None,
            limit: None,
        },
    )
    .unwrap();
    let all_collaterals_res: AllCollateralsResponse = from_binary(&res).unwrap();
    assert_eq!(
        all_collaterals_res,
        AllCollateralsResponse {
            all_collaterals: vec![CollateralsResponse {
                borrower: HumanAddr::from("addr0000"),
                collaterals: vec![
                    (HumanAddr::from("batom"), Uint128::from(10000000u128)),
                    (HumanAddr::from("bluna"), Uint128::from(1000000u128)),
                ]
            }]
        }
    );
}

#[test]
fn unlock_collateral() {
    let mut deps = mock_dependencies(20, &[]);

    let env = mock_env("owner", &[]);
    let msg = InitMsg {
        owner_addr: HumanAddr::from("owner"),
        oracle_contract: HumanAddr::from("oracle"),
        market_contract: HumanAddr::from("market"),
        base_denom: "uusd".to_string(),
        distribution_threshold: Decimal::permille(3),
        target_deposit_rate: Decimal::permille(5),
        buffer_distribution_rate: Decimal::percent(20),
    };

    // we can just call .unwrap() to assert this was a success
    let _res = init(&mut deps, env.clone(), msg).unwrap();

    // store whitelist elems
    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("bluna"),
        custody_contract: HumanAddr::from("custody_bluna"),
        ltv: Decimal::percent(60),
    };

    let _res = handle(&mut deps, env.clone(), msg);

    let msg = HandleMsg::Whitelist {
        collateral_token: HumanAddr::from("batom"),
        custody_contract: HumanAddr::from("custody_batom"),
        ltv: Decimal::percent(60),
    };

    let _res = handle(&mut deps, env.clone(), msg);

    let msg = HandleMsg::LockCollateral {
        collaterals: vec![
            (HumanAddr::from("bluna"), Uint128::from(1000000u128)),
            (HumanAddr::from("batom"), Uint128::from(10000000u128)),
        ],
    };
    let env = mock_env("addr0000", &[]);
    let _res = handle(&mut deps, env.clone(), msg).unwrap();

    // Failed to unlock more than locked amount
    let msg = HandleMsg::UnlockCollateral {
        collaterals: vec![
            (HumanAddr::from("bluna"), Uint128::from(1000001u128)),
            (HumanAddr::from("batom"), Uint128::from(10000001u128)),
        ],
    };
    let res = handle(&mut deps, env.clone(), msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot unlock more than you have")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    deps.querier.with_oracle_price(&[
        (
            &("uusd".to_string(), "bluna".to_string()),
            &(
                Decimal::from_ratio(1000u128, 1u128),
                env.block.time,
                env.block.time,
            ),
        ),
        (
            &("uusd".to_string(), "batom".to_string()),
            &(
                Decimal::from_ratio(2000u128, 1u128),
                env.block.time,
                env.block.time,
            ),
        ),
    ]);

    // borrow_limit = 1000 * 1000000 * 0.6 + 2000 * 10000000 * 0.6
    // = 12,600,000,000 uusd
    deps.querier.with_loan_amount(&[(
        &HumanAddr::from("addr0000"),
        &Uint128::from(12600000000u128),
    )]);

    // cannot unlock any tokens
    // Failed to unlock more than locked amount
    let msg = HandleMsg::UnlockCollateral {
        collaterals: vec![(HumanAddr::from("bluna"), Uint128::from(1u128))],
    };
    let res = handle(&mut deps, env.clone(), msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot unlock collateral more than LTV")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = HandleMsg::UnlockCollateral {
        collaterals: vec![(HumanAddr::from("batom"), Uint128::from(1u128))],
    };
    let res = handle(&mut deps, env.clone(), msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot unlock collateral more than LTV")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // borrow_limit = 1000 * 1000000 * 0.6 + 2000 * 10000000 * 0.6
    // = 12,600,000,000 uusd
    deps.querier.with_loan_amount(&[(
        &HumanAddr::from("addr0000"),
        &Uint128::from(12599999400u128),
    )]);
    let res = query(
        &deps,
        QueryMsg::BorrowLimit {
            borrower: HumanAddr::from("addr0000"),
        },
    )
    .unwrap();
    let borrow_limit_res: BorrowLimitResponse = from_binary(&res).unwrap();
    assert_eq!(
        borrow_limit_res.borrow_limit,
        Uint128::from(12600000000u128),
    );

    // Cannot unlock 2bluna
    let msg = HandleMsg::UnlockCollateral {
        collaterals: vec![(HumanAddr::from("bluna"), Uint128::from(2u128))],
    };
    let res = handle(&mut deps, env.clone(), msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => {
            assert_eq!(msg, "Cannot unlock collateral more than LTV")
        }
        _ => panic!("DO NOT ENTER HERE"),
    }

    // Can unlock 1bluna
    let msg = HandleMsg::UnlockCollateral {
        collaterals: vec![(HumanAddr::from("bluna"), Uint128::from(1u128))],
    };
    let res = handle(&mut deps, env.clone(), msg).unwrap();
    assert_eq!(
        res.messages,
        vec![CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: HumanAddr::from("bluna"),
            send: vec![],
            msg: to_binary(&CustodyHandleMsg::UnlockCollateral {
                borrower: HumanAddr::from("addr0000"),
                amount: Uint128::from(1u128),
            })
            .unwrap(),
        }),]
    );

    assert_eq!(
        res.log,
        vec![
            log("action", "unlock_collateral"),
            log("borrower", "addr0000"),
            log("collaterals", "1bluna"),
        ]
    );
}