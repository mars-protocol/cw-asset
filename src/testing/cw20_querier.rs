use std::collections::HashMap;

use cosmwasm_std::{to_binary, Addr, QuerierResult, SystemError, Uint128};
use cw20::{BalanceResponse, Cw20QueryMsg};

#[derive(Default)]
pub struct Cw20Querier {
    balances: HashMap<Addr, HashMap<Addr, Uint128>>,
}

impl Cw20Querier {
    pub fn handle_query(&self, contract_addr: &Addr, query: Cw20QueryMsg) -> QuerierResult {
        match query {
            Cw20QueryMsg::Balance {
                address,
            } => {
                let contract_balances = match self.balances.get(contract_addr) {
                    Some(balances) => balances,
                    None => {
                        return Err(SystemError::InvalidRequest {
                            error: format!(
                                "[mock]: cw20 balances not set for token {contract_addr:?}",
                            ),
                            request: Default::default(),
                        })
                        .into()
                    },
                };

                let balance = match contract_balances.get(&Addr::unchecked(&address)) {
                    Some(balance) => balance,
                    None => {
                        return Err(SystemError::InvalidRequest {
                            error: format!("[mock]: cw20 balance not set for user {address:?}"),
                            request: Default::default(),
                        })
                        .into()
                    },
                };

                Ok(to_binary(&BalanceResponse {
                    balance: *balance,
                })
                .into())
                .into()
            },

            query => Err(SystemError::InvalidRequest {
                error: format!("[mock]: unsupported cw20 query {query:?}"),
                request: Default::default(),
            })
            .into(),
        }
    }

    pub fn set_balance(&mut self, contract: &str, user: &str, balance: u128) {
        let contract_addr = Addr::unchecked(contract);
        let user_addr = Addr::unchecked(user);

        let contract_balances = self.balances.entry(contract_addr).or_insert_with(HashMap::new);
        contract_balances.insert(user_addr, Uint128::new(balance));
    }
}
