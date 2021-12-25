use cosmwasm_std::{to_binary, Decimal, QuerierResult, SystemError, Uint128};
use std::collections::HashMap;
use terra_cosmwasm::{TaxCapResponse, TaxRateResponse, TerraQuery, TerraRoute};

#[derive(Default)]
pub struct NativeQuerier {
    tax_rate: Decimal,
    tax_caps: HashMap<String, Uint128>,
}

impl NativeQuerier {
    pub fn handle_query(&self, route: &TerraRoute, query_data: &TerraQuery) -> QuerierResult {
        match route {
            TerraRoute::Treasury => self.handle_treasury_query(query_data),

            _ => Err(SystemError::InvalidRequest {
                error: format!("[mock]: unsupported native query route {:?}", route),
                request: Default::default(),
            })
            .into(),
        }
    }

    fn handle_treasury_query(&self, query_data: &TerraQuery) -> QuerierResult {
        match query_data {
            TerraQuery::TaxRate {} => Ok(to_binary(&TaxRateResponse {
                rate: self.tax_rate,
            })
            .into())
            .into(),

            TerraQuery::TaxCap {
                denom,
            } => {
                let cap = match self.tax_caps.get(denom) {
                    Some(cap) => cap,
                    None => {
                        return Err(SystemError::InvalidRequest {
                            error: format!("[mock]: tax cap not set for {:?}", denom),
                            request: Default::default(),
                        })
                        .into()
                    }
                };

                Ok(to_binary(&TaxCapResponse {
                    cap: *cap,
                })
                .into())
                .into()
            }

            _ => Err(SystemError::InvalidRequest {
                error: format!("[mock]: unsupported native query {:?}", query_data),
                request: Default::default(),
            })
            .into(),
        }
    }

    pub fn set_tax_rate(&mut self, tax_rate: Decimal) {
        self.tax_rate = tax_rate;
    }

    pub fn set_tax_cap(&mut self, denom: &str, tax_cap: u128) {
        self.tax_caps.insert(String::from(denom), Uint128::new(tax_cap));
    }
}
