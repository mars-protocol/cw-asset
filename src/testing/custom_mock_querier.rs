use cosmwasm_std::testing::MockQuerier;
use cosmwasm_std::{
    from_binary, from_slice, Addr, Coin, Decimal, Querier, QuerierResult, QueryRequest, StdResult,
    SystemError, WasmQuery,
};
use cw20::Cw20QueryMsg;
use terra_cosmwasm::TerraQueryWrapper;

use super::{cw20_querier::Cw20Querier, native_querier::NativeQuerier};

pub struct CustomMockQuerier {
    base: MockQuerier<TerraQueryWrapper>,
    native_querier: NativeQuerier,
    cw20_querier: Cw20Querier,
}

impl Default for CustomMockQuerier {
    fn default() -> Self {
        CustomMockQuerier {
            base: MockQuerier::new(&[]),
            native_querier: NativeQuerier::default(),
            cw20_querier: Cw20Querier::default(),
        }
    }
}

impl Querier for CustomMockQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<TerraQueryWrapper> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return Err(SystemError::InvalidRequest {
                    error: format!("[mock]: failed to parse query request {}", e),
                    request: bin_request.into(),
                })
                .into()
            }
        };
        self.handle_query(&request)
    }
}

impl CustomMockQuerier {
    pub fn handle_query(&self, request: &QueryRequest<TerraQueryWrapper>) -> QuerierResult {
        match request {
            QueryRequest::Custom(TerraQueryWrapper {
                route,
                query_data,
            }) => self.native_querier.handle_query(route, query_data),

            QueryRequest::Wasm(WasmQuery::Smart {
                contract_addr,
                msg,
            }) => {
                let contract_addr = Addr::unchecked(contract_addr);

                let parse_cw20_query: StdResult<Cw20QueryMsg> = from_binary(msg);
                if let Ok(cw20_query) = parse_cw20_query {
                    return self.cw20_querier.handle_query(&contract_addr, cw20_query);
                }

                panic!("[mock]: unsupported wasm query {:?}", msg);
            }

            _ => self.base.handle_query(request),
        }
    }

    pub fn set_base_balances(&mut self, address: &str, balances: &[Coin]) {
        self.base.update_balance(address, balances.to_vec());
    }

    pub fn set_cw20_balance(&mut self, contract: &str, user: &str, balance: u128) {
        self.cw20_querier.set_balance(contract, user, balance);
    }

    pub fn set_native_tax_rate(&mut self, tax_rate: Decimal) {
        self.native_querier.set_tax_rate(tax_rate);
    }

    pub fn set_native_tax_cap(&mut self, denom: &str, tax_cap: u128) {
        self.native_querier.set_tax_cap(denom, tax_cap);
    }
}
