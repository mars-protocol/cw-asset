use std::fmt;

use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankQuery, QuerierWrapper, QueryRequest, StdError,
    StdResult, Uint128, WasmQuery,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents the type of an fungible asset
///
/// Each **asset info** instance can be one of two variants:
///
/// - CW20 tokens. To create an **asset info** instance of this type, provide the contract address.
/// - Native SDK coins. To create an **asset info** instance of this type, provide the denomination.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfoBase<T> {
    Cw20(T),
    Native(String),
}

impl<T> AssetInfoBase<T> {
    /// Create an **asset info** instance of the _CW20_ variant
    ///
    /// To create an unchecked instance, provide the contract address in any of the following types:
    /// [`cosmwasm_std::Addr`], [`String`], or [`&str`]; to create a checked instance, the address
    /// must of type [`cosmwasm_std::Addr`].
    ///
    /// ```rust
    /// use cosmwasm_std::Addr;
    /// use cw_asset::AssetInfo;
    ///
    /// let info = AssetInfo::cw20(Addr::unchecked("token_addr"));
    /// ```
    pub fn cw20<A: Into<T>>(contract_addr: A) -> Self {
        AssetInfoBase::Cw20(contract_addr.into())
    }

    /// Create an **asset info** instance of the _native_ variant by providing the coin's denomination
    ///
    /// ```rust
    /// use cw_asset::AssetInfo;
    ///
    /// let info = AssetInfo::native("uusd");
    /// ```
    pub fn native<A: Into<String>>(denom: A) -> Self {
        AssetInfoBase::Native(denom.into())
    }
}

/// Represents an **asset info** instance that may contain unverified data; to be used in messages
pub type AssetInfoUnchecked = AssetInfoBase<String>;
/// Represents an **asset info** instance containing only verified data; to be saved in contract storage
pub type AssetInfo = AssetInfoBase<Addr>;

impl From<AssetInfo> for AssetInfoUnchecked {
    fn from(asset_info: AssetInfo) -> Self {
        match &asset_info {
            AssetInfo::Cw20(contract_addr) => AssetInfoUnchecked::Cw20(contract_addr.into()),
            AssetInfo::Native(denom) => AssetInfoUnchecked::Native(denom.clone()),
        }
    }
}

impl AssetInfoUnchecked {
    /// Validate data contained in an _unchecked_ **asset info** instance; return a new _checked_
    /// **asset info** instance
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Api, StdResult};
    /// use cw_asset::{AssetInfo, AssetInfoUnchecked};
    ///
    /// fn validate_asset_info(api: &dyn Api, info_unchecked: &AssetInfoUnchecked) {
    ///     match info_unchecked.check(api) {
    ///         Ok(info) => println!("asset info is valid: {}", info.to_string()),
    ///         Err(err) => println!("asset is invalid! reason: {}", err),
    ///     }
    /// }
    /// ```
    pub fn check(&self, api: &dyn Api) -> StdResult<AssetInfo> {
        Ok(match self {
            AssetInfoUnchecked::Cw20(contract_addr) => {
                AssetInfo::Cw20(api.addr_validate(contract_addr)?)
            }
            AssetInfoUnchecked::Native(denom) => AssetInfo::Native(denom.clone()),
        })
    }

    /// Similar to `check`, but in case `self` is a native token, also verifies its denom is included
    /// in a given whitelist
    pub fn check_whitelist(&self, api: &dyn Api, whitelist: &[&str]) -> StdResult<AssetInfo> {
        Ok(match self {
            AssetInfoUnchecked::Cw20(contract_addr) => {
                AssetInfo::Cw20(api.addr_validate(contract_addr)?)
            }
            AssetInfoUnchecked::Native(denom) => {
                if !whitelist.contains(&&denom[..]) {
                    return Err(StdError::generic_err(
                        format!("invalid denom {}; must be {}", denom, whitelist.join("|"))
                    ));
                }
                AssetInfo::Native(denom.clone())
            }
        })
    }
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetInfo::Cw20(contract_addr) => write!(f, "cw20:{}", contract_addr),
            AssetInfo::Native(denom) => write!(f, "native:{}", denom),
        }
    }
}

impl AssetInfo {
    /// Query an address' balance of the asset
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Deps, StdResult, Uint128};
    /// use cw_asset::AssetInfo;
    ///
    /// fn query_uusd_balance(deps: Deps, account_addr: &Addr) -> StdResult<Uint128> {
    ///     let info = AssetInfo::native("uusd");
    ///     info.query_balance(&deps.querier, "account_addr")
    /// }
    /// ```
    pub fn query_balance<T: Into<String>>(
        &self,
        querier: &QuerierWrapper,
        address: T,
    ) -> StdResult<Uint128> {
        match self {
            AssetInfo::Cw20(contract_addr) => {
                let response: Cw20BalanceResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.into(),
                        msg: to_binary(&Cw20QueryMsg::Balance {
                            address: address.into(),
                        })?,
                    }))?;
                Ok(response.balance)
            }
            AssetInfo::Native(denom) => {
                let response: BalanceResponse =
                    querier.query(&QueryRequest::Bank(BankQuery::Balance {
                        address: address.into(),
                        denom: denom.clone(),
                    }))?;
                Ok(response.amount.amount)
            }
        }
    }
}

#[cfg(feature = "legacy")]
impl From<AssetInfo> for astroport::asset::AssetInfo {
    fn from(info: AssetInfo) -> Self {
        match info {
            AssetInfo::Cw20(contract_addr) => astroport::asset::AssetInfo::Token {
                contract_addr,
            },
            AssetInfo::Native(denom) => astroport::asset::AssetInfo::NativeToken {
                denom,
            },
        }
    }
}

#[cfg(feature = "legacy")]
impl From<&AssetInfo> for astroport::asset::AssetInfo {
    fn from(info: &AssetInfo) -> Self {
        info.clone().into()
    }
}

#[cfg(feature = "legacy")]
impl From<astroport::asset::AssetInfo> for AssetInfo {
    fn from(legacy_info: astroport::asset::AssetInfo) -> Self {
        match legacy_info {
            astroport::asset::AssetInfo::Token {
                contract_addr,
            } => Self::Cw20(contract_addr),
            astroport::asset::AssetInfo::NativeToken {
                denom,
            } => Self::Native(denom),
        }
    }
}

#[cfg(feature = "legacy")]
impl From<&astroport::asset::AssetInfo> for AssetInfo {
    fn from(legacy_info: &astroport::asset::AssetInfo) -> Self {
        legacy_info.clone().into()
    }
}

#[cfg(feature = "legacy")]
impl std::cmp::PartialEq<AssetInfo> for astroport::asset::AssetInfo {
    fn eq(&self, other: &AssetInfo) -> bool {
        match self {
            astroport::asset::AssetInfo::Token {
                contract_addr,
            } => {
                let self_contract_addr = contract_addr;
                match other {
                    AssetInfo::Cw20(contract_addr) => self_contract_addr == contract_addr,
                    _ => false,
                }
            }
            astroport::asset::AssetInfo::NativeToken {
                denom,
            } => {
                let self_denom = denom;
                match other {
                    AssetInfo::Native(denom) => self_denom == denom,
                    _ => false,
                }
            }
        }
    }
}

#[cfg(feature = "legacy")]
impl std::cmp::PartialEq<astroport::asset::AssetInfo> for AssetInfo {
    fn eq(&self, other: &astroport::asset::AssetInfo) -> bool {
        other == self
    }
}

#[cfg(test)]
mod test {
    use super::super::testing::mock_dependencies;
    use super::*;
    use cosmwasm_std::testing::MockApi;
    use cosmwasm_std::Coin;

    #[test]
    fn creating_instances() {
        let info = AssetInfo::cw20(Addr::unchecked("mock_token"));
        assert_eq!(info, AssetInfo::Cw20(Addr::unchecked("mock_token")));

        let info = AssetInfo::native("uusd");
        assert_eq!(info, AssetInfo::Native(String::from("uusd")));
    }

    #[test]
    fn comparing() {
        let uluna = AssetInfo::native("uluna");
        let uusd = AssetInfo::native("uusd");
        let astro = AssetInfo::cw20(Addr::unchecked("astro_token"));
        let mars = AssetInfo::cw20(Addr::unchecked("mars_token"));

        assert_eq!(uluna == uusd, false);
        assert_eq!(uluna == astro, false);
        assert_eq!(astro == mars, false);
        assert_eq!(uluna == uluna.clone(), true);
        assert_eq!(astro == astro.clone(), true);
    }

    #[test]
    fn displaying() {
        let info = AssetInfo::native("uusd");
        assert_eq!(info.to_string(), String::from("native:uusd"));

        let info = AssetInfo::cw20(Addr::unchecked("mock_token"));
        assert_eq!(info.to_string(), String::from("cw20:mock_token"));
    }

    #[test]
    fn checking() {
        let api = MockApi::default();

        let checked = AssetInfo::cw20(Addr::unchecked("mock_token"));
        let unchecked: AssetInfoUnchecked = checked.clone().into();
        assert_eq!(unchecked.check(&api).unwrap(), checked);

        let checked = AssetInfo::native("uusd");
        let unchecked: AssetInfoUnchecked = checked.clone().into();
        assert_eq!(unchecked.check_whitelist(&api, &["uusd", "uluna", "uosmo"]).unwrap(), checked);

        let unchecked = AssetInfoUnchecked::native("uatom");
        assert_eq!(
            unchecked.check_whitelist(&api, &["uusd", "uluna", "uosmo"]), 
            Err(StdError::generic_err("invalid denom uatom; must be uusd|uluna|uosmo")),
        );
    }

    #[test]
    fn querying_balance() {
        let mut deps = mock_dependencies();
        deps.querier.set_base_balances("alice", &[Coin::new(12345, "uusd")]);
        deps.querier.set_cw20_balance("mock_token", "bob", 67890);

        let info1 = AssetInfo::native("uusd");
        let balance1 = info1.query_balance(&deps.as_ref().querier, "alice").unwrap();
        assert_eq!(balance1, Uint128::new(12345));

        let info2 = AssetInfo::cw20(Addr::unchecked("mock_token"));
        let balance2 = info2.query_balance(&deps.as_ref().querier, "bob").unwrap();
        assert_eq!(balance2, Uint128::new(67890));
    }
}

#[cfg(all(test, feature = "legacy"))]
mod tests_legacy {
    use super::*;

    #[test]
    fn casting_legacy() {
        let legacy_info = astroport::asset::AssetInfo::NativeToken {
            denom: String::from("uusd"),
        };

        let info = AssetInfo::native("uusd");

        assert_eq!(info, AssetInfo::from(&legacy_info));
        assert_eq!(info, AssetInfo::from(legacy_info.clone()));
        assert_eq!(legacy_info, astroport::asset::AssetInfo::from(&info));
        assert_eq!(legacy_info, astroport::asset::AssetInfo::from(info));

        let legacy_info = astroport::asset::AssetInfo::Token {
            contract_addr: Addr::unchecked("mock_token"),
        };

        let info = AssetInfo::cw20(Addr::unchecked("mock_token"));

        assert_eq!(info, AssetInfo::from(&legacy_info));
        assert_eq!(info, AssetInfo::from(legacy_info.clone()));
        assert_eq!(legacy_info, astroport::asset::AssetInfo::from(&info));
        assert_eq!(legacy_info, astroport::asset::AssetInfo::from(info));
    }

    #[test]
    fn comparing() {
        let legacy_info_1 = astroport::asset::AssetInfo::NativeToken {
            denom: String::from("uusd"),
        };
        let legacy_info_2 = astroport::asset::AssetInfo::NativeToken {
            denom: String::from("uluna"),
        };
        let legacy_info_3 = astroport::asset::AssetInfo::Token {
            contract_addr: Addr::unchecked("astro_token"),
        };
        let legacy_info_4 = astroport::asset::AssetInfo::Token {
            contract_addr: Addr::unchecked("mars_token"),
        };

        let info_1 = AssetInfo::native("uusd");
        let info_2 = AssetInfo::native("uluna");
        let info_3 = AssetInfo::cw20(Addr::unchecked("astro_token"));
        let info_4 = AssetInfo::cw20(Addr::unchecked("mars_token"));

        assert_eq!(legacy_info_1 == info_1, true);
        assert_eq!(legacy_info_2 == info_1, false);
        assert_eq!(legacy_info_2 == info_2, true);
        assert_eq!(legacy_info_3 == info_1, false);
        assert_eq!(legacy_info_3 == info_3, true);
        assert_eq!(legacy_info_4 == info_3, false);
        assert_eq!(legacy_info_4 == info_4, true);
        assert_eq!(legacy_info_1 == info_4, false);
    }
}
