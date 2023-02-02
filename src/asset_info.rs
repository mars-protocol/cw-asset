use std::{any::type_name, fmt, str::FromStr};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankQuery, QuerierWrapper, QueryRequest, StdError,
    StdResult, Uint128, WasmQuery,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20QueryMsg};
use cw_address_like::AddressLike;
use cw_storage_plus::{Key, KeyDeserialize, Prefixer, PrimaryKey};

use crate::AssetError;

/// Represents the type of an fungible asset.
///
/// Each **asset info** instance can be one of three variants:
///
/// - Native SDK coins. To create an **asset info** instance of this type,
///   provide the denomination.
/// - CW20 tokens. To create an **asset info** instance of this type, provide
///   the contract address.
#[cw_serde]
#[non_exhaustive]
pub enum AssetInfoBase<T: AddressLike> {
    Native(String),
    Cw20(T),
}

impl<T: AddressLike> AssetInfoBase<T> {
    /// Create an **asset info** instance of the _native_ variant by providing
    /// the coin's denomination.
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
    /// ```rust
    /// use cosmwasm_std::Addr;
    /// use cw_asset::AssetInfo;
    ///
    /// let info = AssetInfo::cw20(Addr::unchecked("token_addr"));
    /// ```
    pub fn cw20<A: Into<T>>(contract_addr: A) -> Self {
        AssetInfoBase::Cw20(contract_addr.into())
    }
}

/// Represents an **asset info** instance that may contain unverified data; to
/// be used in messages.
pub type AssetInfoUnchecked = AssetInfoBase<String>;

/// Represents an **asset info** instance containing only verified data; to be
/// saved in contract storage.
pub type AssetInfo = AssetInfoBase<Addr>;

impl FromStr for AssetInfoUnchecked {
    type Err = AssetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let words: Vec<&str> = s.split(':').collect();

        match words[0] {
            "native" => {
                if words.len() != 2 {
                    return Err(AssetError::InvalidAssetInfoFormat {
                        received: s.into(),
                        should_be: "native:{denom}".into(),
                    });
                }
                Ok(AssetInfoUnchecked::Native(String::from(words[1])))
            },
            "cw20" => {
                if words.len() != 2 {
                    return Err(AssetError::InvalidAssetInfoFormat {
                        received: s.into(),
                        should_be: "cw20:{contract_addr}".into(),
                    });
                }
                Ok(AssetInfoUnchecked::Cw20(String::from(words[1])))
            },
            ty => Err(AssetError::InvalidAssetType {
                ty: ty.into(),
            }),
        }
    }
}

impl From<AssetInfo> for AssetInfoUnchecked {
    fn from(asset_info: AssetInfo) -> Self {
        match asset_info {
            AssetInfo::Cw20(contract_addr) => AssetInfoUnchecked::Cw20(contract_addr.into()),
            AssetInfo::Native(denom) => AssetInfoUnchecked::Native(denom),
        }
    }
}

impl From<&AssetInfo> for AssetInfoUnchecked {
    fn from(asset_info: &AssetInfo) -> Self {
        match asset_info {
            AssetInfo::Cw20(contract_addr) => AssetInfoUnchecked::Cw20(contract_addr.into()),
            AssetInfo::Native(denom) => AssetInfoUnchecked::Native(denom.into()),
        }
    }
}

impl AssetInfoUnchecked {
    /// Validate data contained in an _unchecked_ **asset info** instance;
    /// return a new _checked_ **asset info** instance:
    ///
    /// - For CW20 tokens, assert the contract address is valid;
    /// - For SDK coins, assert that the denom is included in a given whitelist;
    ///   skip if the whitelist is not provided.
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
    ) -> Result<AssetInfo, AssetError> {
        match self {
            AssetInfoUnchecked::Native(denom) => {
                if let Some(whitelist) = optional_whitelist {
                    if !whitelist.contains(&&denom[..]) {
                        return Err(AssetError::UnacceptedDenom {
                            denom: denom.clone(),
                            whitelist: whitelist.join("|"),
                        });
                    }
                }
                Ok(AssetInfo::Native(denom.clone()))
            },
            AssetInfoUnchecked::Cw20(contract_addr) => {
                Ok(AssetInfo::Cw20(api.addr_validate(contract_addr)?))
            },
        }
    }
}

impl fmt::Display for AssetInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetInfo::Cw20(contract_addr) => write!(f, "cw20:{contract_addr}"),
            AssetInfo::Native(denom) => write!(f, "native:{denom}"),
        }
    }
}

impl AssetInfo {
    /// Query an address' balance of the asset
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Deps, Uint128};
    /// use cw_asset::{AssetError, AssetInfo};
    ///
    /// fn query_uusd_balance(deps: Deps, account_addr: &Addr) -> Result<Uint128, AssetError> {
    ///     let info = AssetInfo::native("uusd");
    ///     info.query_balance(&deps.querier, "account_addr")
    /// }
    /// ```
    pub fn query_balance<T: Into<String>>(
        &self,
        querier: &QuerierWrapper,
        address: T,
    ) -> Result<Uint128, AssetError> {
        match self {
            AssetInfo::Native(denom) => {
                let response: BalanceResponse =
                    querier.query(&QueryRequest::Bank(BankQuery::Balance {
                        address: address.into(),
                        denom: denom.clone(),
                    }))?;
                Ok(response.amount.amount)
            },
            AssetInfo::Cw20(contract_addr) => {
                let response: Cw20BalanceResponse =
                    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
                        contract_addr: contract_addr.into(),
                        msg: to_binary(&Cw20QueryMsg::Balance {
                            address: address.into(),
                        })?,
                    }))?;
                Ok(response.balance)
            },
        }
    }

    /// Implemented as private function to prevent from_str from being called on AssetInfo
    fn from_str(s: &str) -> Result<Self, AssetError> {
        let words: Vec<&str> = s.split(':').collect();

        match words[0] {
            "native" => {
                if words.len() != 2 {
                    return Err(AssetError::InvalidAssetInfoFormat {
                        received: s.into(),
                        should_be: "native:{denom}".into(),
                    });
                }
                Ok(AssetInfo::Native(String::from(words[1])))
            },
            "cw20" => {
                if words.len() != 2 {
                    return Err(AssetError::InvalidAssetInfoFormat {
                        received: s.into(),
                        should_be: "cw20:{contract_addr}".into(),
                    });
                }
                Ok(AssetInfo::Cw20(Addr::unchecked(words[1])))
            },
            ty => Err(AssetError::InvalidAssetType {
                ty: ty.into(),
            }),
        }
    }
}

impl<'a> PrimaryKey<'a> for &AssetInfo {
    type Prefix = ();
    type SubPrefix = ();
    type Suffix = Self;
    type SuperSuffix = Self;

    fn key(&self) -> Vec<Key> {
        let mut keys = vec![];
        match &self {
            AssetInfo::Cw20(addr) => {
                keys.extend("cw20:".key());
                keys.extend(addr.key());
            },
            AssetInfo::Native(denom) => {
                keys.extend("native:".key());
                keys.extend(denom.key());
            },
        };
        keys
    }
}

impl KeyDeserialize for &AssetInfo {
    type Output = AssetInfo;

    #[inline(always)]
    fn from_vec(mut value: Vec<u8>) -> StdResult<Self::Output> {
        // ignore length prefix
        // we're allowed to do this because we set the key's namespace ourselves
        // in PrimaryKey (first key)
        value.drain(0..2);

        // parse the bytes into an utf8 string
        let s = String::from_utf8(value)?;

        // cast the AssetError to StdError::ParseError
        AssetInfo::from_str(&s)
            .map_err(|err| StdError::parse_err(type_name::<Self::Output>(), err))
    }
}

impl<'a> Prefixer<'a> for &AssetInfo {
    fn prefix(&self) -> Vec<Key> {
        self.key()
    }
}

//------------------------------------------------------------------------------
// Tests
//------------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use cosmwasm_std::{testing::MockApi, Coin};

    use super::{super::testing::mock_dependencies, *};

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

        assert!(uluna != uusd);
        assert!(uluna != astro);
        assert!(astro != mars);
        assert!(uluna == uluna.clone());
        assert!(astro == astro.clone());
    }

    #[test]
    fn from_string() {
        let s = "";
        assert_eq!(
            AssetInfoUnchecked::from_str(s),
            Err(AssetError::InvalidAssetType {
                ty: "".into()
            }),
        );

        let s = "native:uusd:12345";
        assert_eq!(
            AssetInfoUnchecked::from_str(s),
            Err(AssetError::InvalidAssetInfoFormat {
                received: s.into(),
                should_be: "native:{denom}".into(),
            }),
        );

        let s = "cw721:galactic_punk";
        assert_eq!(
            AssetInfoUnchecked::from_str(s),
            Err(AssetError::InvalidAssetType {
                ty: "cw721".into(),
            })
        );

        let s = "native:uusd";
        assert_eq!(AssetInfoUnchecked::from_str(s).unwrap(), AssetInfoUnchecked::native("uusd"),);

        let s = "cw20:mock_token";
        assert_eq!(
            AssetInfoUnchecked::from_str(s).unwrap(),
            AssetInfoUnchecked::cw20("mock_token"),
        );
    }

    #[test]
    fn to_string() {
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
        assert_eq!(unchecked.check(&api, None).unwrap(), checked);

        let checked = AssetInfo::native("uusd");
        let unchecked: AssetInfoUnchecked = checked.clone().into();
        assert_eq!(unchecked.check(&api, Some(&["uusd", "uluna", "uosmo"])).unwrap(), checked);

        let unchecked = AssetInfoUnchecked::native("uatom");
        assert_eq!(
            unchecked.check(&api, Some(&["uusd", "uluna", "uosmo"])),
            Err(AssetError::UnacceptedDenom {
                denom: "uatom".into(),
                whitelist: "uusd|uluna|uosmo".into(),
            }),
        );
    }

    #[test]
    fn checking_uppercase() {
        let api = MockApi::default();

        let unchecked = AssetInfoUnchecked::cw20("TERRA1234ABCD");
        assert_eq!(
            unchecked.check(&api, None).unwrap_err(),
            StdError::generic_err("Invalid input: address not normalized").into(),
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

    use cosmwasm_std::{Addr, Order};
    use cw_storage_plus::Map;

    fn mock_key() -> AssetInfo {
        AssetInfo::native("uusd")
    }

    fn mock_keys() -> (AssetInfo, AssetInfo, AssetInfo) {
        (
            AssetInfo::native("uusd"),
            AssetInfo::cw20(Addr::unchecked("mock_token")),
            AssetInfo::cw20(Addr::unchecked("mock_token2")),
        )
    }

    #[test]
    fn storage_key_works() {
        let mut deps = mock_dependencies();
        let key = mock_key();
        let map: Map<&AssetInfo, u64> = Map::new("map");

        map.save(deps.as_mut().storage, &key, &42069).unwrap();

        assert_eq!(map.load(deps.as_ref().storage, &key).unwrap(), 42069);

        let items = map
            .range(deps.as_ref().storage, None, None, Order::Ascending)
            .map(|item| item.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0], (key, 42069));
    }

    #[test]
    fn composite_key_works() {
        let mut deps = mock_dependencies();
        let key = mock_key();
        let map: Map<(&AssetInfo, Addr), u64> = Map::new("map");

        map.save(deps.as_mut().storage, (&key, Addr::unchecked("larry")), &42069).unwrap();

        map.save(deps.as_mut().storage, (&key, Addr::unchecked("jake")), &69420).unwrap();

        let items = map
            .prefix(&key)
            .range(deps.as_ref().storage, None, None, Order::Ascending)
            .map(|item| item.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0], (Addr::unchecked("jake"), 69420));
        assert_eq!(items[1], (Addr::unchecked("larry"), 42069));
    }

    #[test]
    fn triple_asset_key_works() {
        let mut deps = mock_dependencies();
        let map: Map<(&AssetInfo, &AssetInfo, &AssetInfo), u64> = Map::new("map");

        let (key1, key2, key3) = mock_keys();
        map.save(deps.as_mut().storage, (&key1, &key2, &key3), &42069).unwrap();
        map.save(deps.as_mut().storage, (&key1, &key1, &key2), &11).unwrap();
        map.save(deps.as_mut().storage, (&key1, &key1, &key3), &69420).unwrap();

        let items = map
            .prefix((&key1, &key1))
            .range(deps.as_ref().storage, None, None, Order::Ascending)
            .map(|item| item.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], (key3.clone(), 69420));
        assert_eq!(items[0], (key2.clone(), 11));

        let val1 = map.load(deps.as_ref().storage, (&key1, &key2, &key3)).unwrap();
        assert_eq!(val1, 42069);
    }
}
