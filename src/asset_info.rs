use std::fmt;
use std::str::FromStr;

use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankQuery, QuerierWrapper, QueryRequest, StdError,
    StdResult, Uint128, WasmQuery,
};
use cw1155::{BalanceResponse as Cw1155BalanceResponse, Cw1155QueryMsg};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents the type of an fungible asset
///
/// Each **asset info** instance can be one of three variants:
///
/// - Native SDK coins. To create an **asset info** instance of this type, provide the denomination.
/// - CW20 tokens. To create an **asset info** instance of this type, provide the contract address.
/// - CW1155 tokens. To create an **asset info** instance of this type, provide the contract address and token ID.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfoBase<T> {
    Native(String),
    Cw20(T),
    Cw1155(T, String),
}

impl<T> AssetInfoBase<T> {
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

    /// Create an **asset info** instance of the _cw1155_ variant by providing the
    /// contract address and token_id
    ///
    /// ```rust
    /// use cosmwasm_std::Addr;
    /// use cw_asset::AssetInfo;
    ///
    /// let info = AssetInfo::cw1155(Addr::unchecked("token_addr"), "uusd");
    /// ```
    pub fn cw1155<A: Into<T>, B: Into<String>>(contract_addr: A, token_id: B) -> Self {
        AssetInfoBase::Cw1155(contract_addr.into(), token_id.into())
    }
}

/// Represents an **asset info** instance that may contain unverified data; to be used in messages
pub type AssetInfoUnchecked = AssetInfoBase<String>;
/// Represents an **asset info** instance containing only verified data; to be saved in contract storage
pub type AssetInfo = AssetInfoBase<Addr>;

impl FromStr for AssetInfoUnchecked {
    type Err = StdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let words: Vec<&str> = s.split(':').collect();

        match words[0] {
            "native" => {
                if words.len() != 2 {
                    return Err(StdError::generic_err(format!(
                        "invalid asset info format `{}`; must be in format `native:{{denom}}`",
                        s
                    )));
                }
                Ok(AssetInfoUnchecked::Native(String::from(words[1])))
            }
            "cw20" => {
                if words.len() != 2 {
                    return Err(StdError::generic_err(
                        format!("invalid asset info format `{}`; must be in format `cw20:{{contract_addr}}`", s)
                    ));
                }
                Ok(AssetInfoUnchecked::Cw20(String::from(words[1])))
            }
            "cw1155" => {
                if words.len() != 3 {
                    return Err(StdError::generic_err(
                        format!("invalid asset info format `{}`; must be in format `cw1155:{{contract_addr}}:{{token_id}}`", s)
                    ));
                }
                Ok(AssetInfoUnchecked::Cw1155(
                    String::from(words[1]),
                    String::from(words[2]),
                ))
            }
            ty => Err(StdError::generic_err(format!(
                "invalid asset type `{}`; must be `native` or `cw20` or `cw1155`",
                ty
            ))),
        }
    }
}

impl From<AssetInfo> for AssetInfoUnchecked {
    fn from(asset_info: AssetInfo) -> Self {
        match asset_info {
            AssetInfo::Cw20(contract_addr) => AssetInfoUnchecked::Cw20(contract_addr.into()),
            AssetInfo::Native(denom) => AssetInfoUnchecked::Native(denom),
            AssetInfo::Cw1155(contract_addr, token_id) => {
                AssetInfoUnchecked::Cw1155(contract_addr.into(), token_id)
            }
        }
    }
}

impl From<&AssetInfo> for AssetInfoUnchecked {
    fn from(asset_info: &AssetInfo) -> Self {
        match asset_info {
            AssetInfo::Cw20(contract_addr) => AssetInfoUnchecked::Cw20(contract_addr.into()),
            AssetInfo::Native(denom) => AssetInfoUnchecked::Native(denom.into()),
            AssetInfo::Cw1155(contract_addr, token_id) => {
                AssetInfoUnchecked::Cw1155(contract_addr.into(), token_id.into())
            }
        }
    }
}

impl AssetInfoUnchecked {
    /// Validate data contained in an _unchecked_ **asset info** instance; return a new _checked_
    /// **asset info** instance:
    /// * For CW20 tokens, assert the contract address is valid
    /// * For SDK coins, assert that the denom is included in a given whitelist; skip if the
    ///   whitelist is not provided
    ///
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Api, StdResult};
    /// use cw_asset::{AssetInfo, AssetInfoUnchecked};
    ///
    /// fn validate_asset_info(api: &dyn Api, info_unchecked: &AssetInfoUnchecked) {
    ///     match info_unchecked.check(api, Some(&["uatom", "uluna"])) {
    ///         Ok(info) => println!("asset info is valid: {}", info.to_string()),
    ///         Err(err) => println!("asset is invalid! reason: {}", err),
    ///     }
    /// }
    /// ```
    pub fn check(
        &self,
        api: &dyn Api,
        optional_whitelist: Option<&[&str]>,
    ) -> StdResult<AssetInfo> {
        Ok(match self {
            AssetInfoUnchecked::Native(denom) => {
                if let Some(whitelist) = optional_whitelist {
                    if !whitelist.contains(&&denom[..]) {
                        return Err(StdError::generic_err(format!(
                            "invalid denom {}; must be {}",
                            denom,
                            whitelist.join("|")
                        )));
                    }
                }
                AssetInfo::Native(denom.clone())
            }
            AssetInfoUnchecked::Cw20(contract_addr) => {
                AssetInfo::Cw20(api.addr_validate(contract_addr)?)
            }
            AssetInfoUnchecked::Cw1155(contract_addr, token_id) => {
                if token_id.is_empty() {
                    return Err(StdError::generic_err("token_id must not be empty"));
                }
                AssetInfo::Cw1155(api.addr_validate(contract_addr)?, token_id.clone())
            }
        })
    }
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetInfo::Cw20(contract_addr) => write!(f, "cw20:{}", contract_addr),
            AssetInfo::Native(denom) => write!(f, "native:{}", denom),
            AssetInfo::Cw1155(contract_addr, token_id) => {
                write!(f, "cw1155:{}:{}", contract_addr, token_id)
            }
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
            AssetInfo::Native(denom) => {
                let response: BalanceResponse =
                    querier.query(&QueryRequest::Bank(BankQuery::Balance {
                        address: address.into(),
                        denom: denom.clone(),
                    }))?;
                Ok(response.amount.amount)
            }
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
            AssetInfo::Cw1155(contract_addr, token_id) => {
                let response: Cw1155BalanceResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.into(),
                        msg: to_binary(&Cw1155QueryMsg::Balance {
                            owner: address.into(),
                            token_id: token_id.into(),
                        })?,
                    }))?;
                Ok(response.balance)
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

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

        let info = AssetInfo::cw1155(Addr::unchecked("mock_contract"), "mock_token");
        assert_eq!(
            info,
            AssetInfo::Cw1155(Addr::unchecked("mock_contract"), String::from("mock_token"))
        );
    }

    #[test]
    fn comparing() {
        let uluna = AssetInfo::native("uluna");
        let uusd = AssetInfo::native("uusd");
        let astro = AssetInfo::cw20(Addr::unchecked("astro_token"));
        let mars = AssetInfo::cw20(Addr::unchecked("mars_token"));
        let cw1155_x = AssetInfo::cw1155(Addr::unchecked("cw1155"), String::from("x"));
        let cw1155_y = AssetInfo::cw1155(Addr::unchecked("cw1155"), String::from("y"));

        assert_eq!(uluna == uusd, false);
        assert_eq!(uluna == astro, false);
        assert_eq!(astro == mars, false);
        assert_eq!(uluna == uluna.clone(), true);
        assert_eq!(astro == astro.clone(), true);
        assert_eq!(cw1155_x == cw1155_y, false);
        assert_eq!(cw1155_x == cw1155_x.clone(), true);
    }

    #[test]
    fn from_string() {
        let s = "";
        assert_eq!(
            AssetInfoUnchecked::from_str(s),
            Err(StdError::generic_err(
                "invalid asset type ``; must be `native` or `cw20` or `cw1155`"
            )),
        );

        let s = "native:uusd:12345";
        assert_eq!(
            AssetInfoUnchecked::from_str(s),
            Err(StdError::generic_err(
                "invalid asset info format `native:uusd:12345`; must be in format `native:{denom}`"
            )),
        );

        let s = "cw721:galactic_punk";
        assert_eq!(
            AssetInfoUnchecked::from_str(s),
            Err(StdError::generic_err(
                "invalid asset type `cw721`; must be `native` or `cw20` or `cw1155`"
            )),
        );

        let s = "native:uusd";
        assert_eq!(
            AssetInfoUnchecked::from_str(s).unwrap(),
            AssetInfoUnchecked::native("uusd"),
        );

        let s = "cw20:mock_token";
        assert_eq!(
            AssetInfoUnchecked::from_str(s).unwrap(),
            AssetInfoUnchecked::cw20("mock_token"),
        );

        let s = "cw1155:mock_contract:mock_token";
        assert_eq!(
            AssetInfoUnchecked::from_str(s).unwrap(),
            AssetInfoUnchecked::cw1155("mock_contract", "mock_token"),
        );
    }

    #[test]
    fn to_string() {
        let info = AssetInfo::native("uusd");
        assert_eq!(info.to_string(), String::from("native:uusd"));

        let info = AssetInfo::cw20(Addr::unchecked("mock_token"));
        assert_eq!(info.to_string(), String::from("cw20:mock_token"));

        let info = AssetInfo::cw1155(Addr::unchecked("mock_contract"), "mock_token");
        assert_eq!(
            info.to_string(),
            String::from("cw1155:mock_contract:mock_token")
        );
    }

    #[test]
    fn checking() {
        let api = MockApi::default();

        let checked = AssetInfo::cw20(Addr::unchecked("mock_token"));
        let unchecked: AssetInfoUnchecked = checked.clone().into();
        assert_eq!(unchecked.check(&api, None).unwrap(), checked);

        let checked = AssetInfo::native("uusd");
        let unchecked: AssetInfoUnchecked = checked.clone().into();
        assert_eq!(
            unchecked
                .check(&api, Some(&["uusd", "uluna", "uosmo"]))
                .unwrap(),
            checked
        );

        let unchecked = AssetInfoUnchecked::native("uatom");
        assert_eq!(
            unchecked.check(&api, Some(&["uusd", "uluna", "uosmo"])),
            Err(StdError::generic_err(
                "invalid denom uatom; must be uusd|uluna|uosmo"
            )),
        );
    }

    #[test]
    fn checking_uppercase() {
        let api = MockApi::default();

        let unchecked = AssetInfoUnchecked::cw20("TERRA1234ABCD");
        assert_eq!(
            unchecked.check(&api, None).unwrap_err(),
            StdError::generic_err("Invalid input: address not normalized"),
        );

        let unchecked = AssetInfoUnchecked::cw1155("TERRA1234ABCD", "mock_token");
        assert_eq!(
            unchecked.check(&api, None).unwrap_err(),
            StdError::generic_err("Invalid input: address not normalized"),
        );
    }

    #[test]
    fn querying_balance() {
        let mut deps = mock_dependencies();
        deps.querier
            .set_base_balances("alice", &[Coin::new(12345, "uusd")]);
        deps.querier.set_cw20_balance("mock_token", "bob", 67890);

        let info1 = AssetInfo::native("uusd");
        let balance1 = info1
            .query_balance(&deps.as_ref().querier, "alice")
            .unwrap();
        assert_eq!(balance1, Uint128::new(12345));

        let info2 = AssetInfo::cw20(Addr::unchecked("mock_token"));
        let balance2 = info2.query_balance(&deps.as_ref().querier, "bob").unwrap();
        assert_eq!(balance2, Uint128::new(67890));
    }
}
