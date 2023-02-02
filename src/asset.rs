use std::{convert::TryFrom, fmt, str::FromStr};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Addr, Api, BankMsg, Binary, Coin, CosmosMsg, Uint128, WasmMsg};
use cw20::Cw20ExecuteMsg;
use cw_address_like::AddressLike;

use crate::{AssetError, AssetInfo, AssetInfoBase, AssetInfoUnchecked};

/// Represents a fungible asset with a known amount
///
/// Each asset instance contains two values: `info`, which specifies the asset's
/// type (CW20 or native), and its `amount`, which specifies the asset's amount.
#[cw_serde]
pub struct AssetBase<T: AddressLike> {
    /// Specifies the asset's type (CW20 or native)
    pub info: AssetInfoBase<T>,
    /// Specifies the asset's amount
    pub amount: Uint128,
}

impl<T: AddressLike> AssetBase<T> {
    /// Create a new **asset** instance based on given asset info and amount
    ///
    /// To create an unchecked instance, the `info` parameter may be either
    /// checked or unchecked; to create a checked instance, the `info` paramter
    /// must also be checked.
    ///
    /// ```rust
    /// use cosmwasm_std::Addr;
    /// use cw_asset::{Asset, AssetInfo};
    ///
    /// let info1 = AssetInfo::cw20(Addr::unchecked("token_addr"));
    /// let asset1 = Asset::new(info1, 12345u128);
    ///
    /// let info2 = AssetInfo::native("uusd");
    /// let asset2 = Asset::new(info2, 67890u128);
    /// ```
    pub fn new<A: Into<AssetInfoBase<T>>, B: Into<Uint128>>(info: A, amount: B) -> Self {
        Self {
            info: info.into(),
            amount: amount.into(),
        }
    }

    /// Create a new **asset** instance representing a native coin of given denom and amount
    ///
    /// ```rust
    /// use cw_asset::Asset;
    ///
    /// let asset = Asset::native("uusd", 12345u128);
    /// ```
    pub fn native<A: Into<String>, B: Into<Uint128>>(denom: A, amount: B) -> Self {
        Self {
            info: AssetInfoBase::native(denom),
            amount: amount.into(),
        }
    }

    /// Create a new **asset** instance representing a CW20 token of given
    /// contract address and amount.
    ///
    /// ```rust
    /// use cosmwasm_std::Addr;
    /// use cw_asset::Asset;
    ///
    /// let asset = Asset::cw20(Addr::unchecked("token_addr"), 12345u128);
    /// ```
    pub fn cw20<A: Into<T>, B: Into<Uint128>>(contract_addr: A, amount: B) -> Self {
        Self {
            info: AssetInfoBase::cw20(contract_addr),
            amount: amount.into(),
        }
    }
}

// Represents an **asset** instance that may contain unverified data; to be used
// in messages.
pub type AssetUnchecked = AssetBase<String>;

// Represents an **asset** instance containing only verified data; to be saved
// in contract storage.
pub type Asset = AssetBase<Addr>;

impl FromStr for AssetUnchecked {
    type Err = AssetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let words: Vec<&str> = s.split(':').collect();

        let info = match words[0] {
            "native" | "cw20" => {
                if words.len() != 3 {
                    return Err(AssetError::InvalidAssetFormat {
                        received: s.into(),
                    });
                }
                AssetInfoUnchecked::from_str(&format!("{}:{}", words[0], words[1]))?
            },
            ty => {
                return Err(AssetError::InvalidAssetType {
                    ty: ty.into(),
                });
            },
        };

        let amount_str = words[words.len() - 1];
        let amount = Uint128::from_str(amount_str).map_err(|_| AssetError::InvalidAssetAmount {
            amount: amount_str.into(),
        })?;

        Ok(AssetUnchecked {
            info,
            amount,
        })
    }
}

impl From<Asset> for AssetUnchecked {
    fn from(asset: Asset) -> Self {
        AssetUnchecked {
            info: asset.info.into(),
            amount: asset.amount,
        }
    }
}

impl AssetUnchecked {
    /// Parse a string of the format `{amount}{denom}` into an `AssetUnchecked`
    /// object. This is the format that Cosmos SDK uses to stringify native
    /// coins. For example:
    ///
    /// - `12345uatom`
    /// - `69420ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2`
    /// - `88888factory/osmo1z926ax906k0ycsuckele6x5hh66e2m4m6ry7dn`
    ///
    /// Since native coin denoms can only start with a non-numerial character,
    /// while its amount can only contain numerical characters, we simply
    /// consider the first non-numerical character and all that comes after as
    /// the denom, while all that comes before it as the amount. This is the
    /// approach used in the [Steak Hub contract](https://github.com/st4k3h0us3/steak-contracts/blob/v1.0.0/contracts/hub/src/helpers.rs#L48-L68).
    pub fn from_sdk_string(s: &str) -> Result<Self, AssetError> {
        for (i, c) in s.chars().enumerate() {
            if !c.is_ascii_digit() {
                let amount = Uint128::from_str(&s[..i])?;
                let denom = &s[i..];
                return Ok(Self::native(denom, amount));
            }
        }

        Err(AssetError::InvalidSdkCoin {
            coin_str: s.into(),
        })
    }

    /// Validate data contained in an _unchecked_ **asset** instnace, return a
    /// new _checked_ **asset** instance:
    ///
    /// - For CW20 tokens, assert the contract address is valid;
    /// - For SDK coins, assert that the denom is included in a given whitelist;
    ///   skip if the whitelist is not provided.
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Api};
    /// use cw_asset::{Asset, AssetUnchecked};
    ///
    /// fn validate_asset(api: &dyn Api, asset_unchecked: &AssetUnchecked) {
    ///     match asset_unchecked.check(api, Some(&["uatom", "uluna"])) {
    ///         Ok(asset) => println!("asset is valid: {}", asset.to_string()),
    ///         Err(err) => println!("asset is invalid! reason: {}", err),
    ///     }
    /// }
    /// ```
    pub fn check(
        &self,
        api: &dyn Api,
        optional_whitelist: Option<&[&str]>,
    ) -> Result<Asset, AssetError> {
        Ok(Asset {
            info: self.info.check(api, optional_whitelist)?,
            amount: self.amount,
        })
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.info, self.amount)
    }
}

impl From<Coin> for Asset {
    fn from(coin: Coin) -> Self {
        Self {
            info: AssetInfo::Native(coin.denom),
            amount: coin.amount,
        }
    }
}

impl From<&Coin> for Asset {
    fn from(coin: &Coin) -> Self {
        coin.clone().into()
    }
}

impl TryFrom<Asset> for Coin {
    type Error = AssetError;

    fn try_from(asset: Asset) -> Result<Self, Self::Error> {
        match &asset.info {
            AssetInfo::Native(denom) => Ok(Coin {
                denom: denom.clone(),
                amount: asset.amount,
            }),
            AssetInfo::Cw20(_) => Err(AssetError::CannotCastToStdCoin {
                asset: asset.to_string(),
            }),
        }
    }
}

impl TryFrom<&Asset> for Coin {
    type Error = AssetError;

    fn try_from(asset: &Asset) -> Result<Self, Self::Error> {
        Coin::try_from(asset.clone())
    }
}

impl std::cmp::PartialEq<Asset> for Coin {
    fn eq(&self, other: &Asset) -> bool {
        match &other.info {
            AssetInfo::Native(denom) => self.denom == *denom && self.amount == other.amount,
            AssetInfo::Cw20(_) => false,
        }
    }
}

impl std::cmp::PartialEq<Coin> for Asset {
    fn eq(&self, other: &Coin) -> bool {
        other == self
    }
}

impl Asset {
    /// Generate a message that sends a CW20 token to the specified recipient
    /// with a binary payload.
    ///
    /// NOTE: Only works for CW20 tokens. Returns error if invoked on an `Asset`
    /// instance representing a native coin, as native coins do not have an
    /// equivalent method mplemented.
    ///
    /// ```rust
    /// use serde::Serialize;
    ///
    /// #[derive(Serialize)]
    /// enum MockReceiveMsg {
    ///     MockCommand {},
    /// }
    ///
    /// use cosmwasm_std::{to_binary, Addr, Response};
    /// use cw_asset::{Asset, AssetError};
    ///
    /// fn send_asset(
    ///     asset: &Asset,
    ///     contract_addr: &Addr,
    ///     msg: &MockReceiveMsg,
    /// ) -> Result<Response, AssetError> {
    ///     let msg = asset.send_msg(contract_addr, to_binary(msg)?)?;
    ///
    ///     Ok(Response::new()
    ///         .add_message(msg)
    ///         .add_attribute("asset_sent", asset.to_string()))
    /// }
    /// ```
    pub fn send_msg<A: Into<String>>(&self, to: A, msg: Binary) -> Result<CosmosMsg, AssetError> {
        match &self.info {
            AssetInfo::Cw20(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.into(),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: to.into(),
                    amount: self.amount,
                    msg,
                })?,
                funds: vec![],
            })),
            AssetInfo::Native(_) => Err(AssetError::UnavailableMethodForNative {
                method: "send".into(),
            }),
        }
    }

    /// Generate a message that transfers the asset from the sender to to a
    /// specified account.
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Response};
    /// use cw_asset::{Asset, AssetError};
    ///
    /// fn transfer_asset(asset: &Asset, recipient_addr: &Addr) -> Result<Response, AssetError> {
    ///     let msg = asset.transfer_msg(recipient_addr)?;
    ///
    ///     Ok(Response::new()
    ///         .add_message(msg)
    ///         .add_attribute("asset_sent", asset.to_string()))
    /// }
    /// ```
    pub fn transfer_msg<A: Into<String>>(&self, to: A) -> Result<CosmosMsg, AssetError> {
        match &self.info {
            AssetInfo::Native(denom) => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: to.into(),
                amount: vec![Coin {
                    denom: denom.clone(),
                    amount: self.amount,
                }],
            })),
            AssetInfo::Cw20(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.into(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: to.into(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
        }
    }

    /// Generate a message that draws the asset from the account specified by
    /// `from` to the one specified by `to`.
    ///
    /// NOTE: Only works for CW20 tokens. Returns error if invoked on an `Asset`
    /// instance representing a native coin, as native coins do not have an
    /// equivalent method implemented.
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Response};
    /// use cw_asset::{Asset, AssetError};
    ///
    /// fn draw_asset(
    ///     asset: &Asset,
    ///     user_addr: &Addr,
    ///     contract_addr: &Addr,
    /// ) -> Result<Response, AssetError> {
    ///     let msg = asset.transfer_from_msg(user_addr, contract_addr)?;
    ///
    ///     Ok(Response::new()
    ///         .add_message(msg)
    ///         .add_attribute("asset_drawn", asset.to_string()))
    /// }
    /// ```
    pub fn transfer_from_msg<A: Into<String>, B: Into<String>>(
        &self,
        from: A,
        to: B,
    ) -> Result<CosmosMsg, AssetError> {
        match &self.info {
            AssetInfo::Cw20(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.into(),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: from.into(),
                    recipient: to.into(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::Native(_) => Err(AssetError::UnavailableMethodForNative {
                method: "transfer_from".into(),
            }),
        }
    }
}

//------------------------------------------------------------------------------
// Tests
//------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use cosmwasm_std::{testing::MockApi, StdError};
    use serde::Serialize;

    use super::*;
    use crate::AssetInfoUnchecked;

    #[derive(Serialize)]
    enum MockExecuteMsg {
        MockCommand {},
    }

    #[test]
    fn creating_instances() {
        let info = AssetInfo::native("uusd");
        let asset = Asset::new(info, 123456u128);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Native(String::from("uusd")),
                amount: Uint128::new(123456u128)
            },
        );

        let asset = Asset::cw20(Addr::unchecked("mock_token"), 123456u128);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("mock_token")),
                amount: Uint128::new(123456u128)
            },
        );

        let asset = Asset::native("uusd", 123456u128);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Native(String::from("uusd")),
                amount: Uint128::new(123456u128)
            },
        )
    }

    #[test]
    fn casting_coin() {
        let uusd = Asset::native("uusd", 69u128);
        let uusd_coin = Coin {
            denom: String::from("uusd"),
            amount: Uint128::new(69),
        };
        assert_eq!(Coin::try_from(&uusd).unwrap(), uusd_coin);
        assert_eq!(Coin::try_from(uusd).unwrap(), uusd_coin);

        let astro = Asset::cw20(Addr::unchecked("astro_token"), 69u128);
        assert_eq!(
            Coin::try_from(&astro),
            Err(AssetError::CannotCastToStdCoin {
                asset: "cw20:astro_token:69".into(),
            }),
        );
        assert_eq!(
            Coin::try_from(astro),
            Err(AssetError::CannotCastToStdCoin {
                asset: "cw20:astro_token:69".into(),
            }),
        );
    }

    #[test]
    fn comparing() {
        let uluna1 = Asset::native("uluna", 69u128);
        let uluna2 = Asset::native("uluna", 420u128);
        let uusd = Asset::native("uusd", 69u128);
        let astro = Asset::cw20(Addr::unchecked("astro_token"), 69u128);

        assert!(uluna1 != uluna2);
        assert!(uluna1 != uusd);
        assert!(astro == astro.clone());
    }

    #[test]
    fn comparing_coin() {
        let uluna = Asset::native("uluna", 69u128);
        let uusd_1 = Asset::native("uusd", 69u128);
        let uusd_2 = Asset::native("uusd", 420u128);
        let uusd_coin = Coin {
            denom: String::from("uusd"),
            amount: Uint128::new(69),
        };
        let astro = Asset::cw20(Addr::unchecked("astro_token"), 69u128);

        assert!(uluna != uusd_coin);
        assert!(uusd_coin != uluna);
        assert!(uusd_1 == uusd_coin);
        assert!(uusd_coin == uusd_1);
        assert!(uusd_2 != uusd_coin);
        assert!(uusd_coin != uusd_2);
        assert!(astro != uusd_coin);
        assert!(uusd_coin != astro);
    }

    #[test]
    fn from_string() {
        let s = "";
        assert_eq!(
            AssetUnchecked::from_str(s),
            Err(AssetError::InvalidAssetType {
                ty: "".into(),
            }),
        );

        let s = "native:uusd:12345:67890";
        assert_eq!(
            AssetUnchecked::from_str(s),
            Err(AssetError::InvalidAssetFormat {
                received: s.into(),
            }),
        );

        let s = "cw721:galactic_punk:1";
        assert_eq!(
            AssetUnchecked::from_str(s),
            Err(AssetError::InvalidAssetType {
                ty: "cw721".into(),
            }),
        );

        let s = "native:uusd:ngmi";
        assert_eq!(
            AssetUnchecked::from_str(s),
            Err(AssetError::InvalidAssetAmount {
                amount: "ngmi".into(),
            }),
        );

        let s = "native:uusd:12345";
        assert_eq!(AssetUnchecked::from_str(s).unwrap(), AssetUnchecked::native("uusd", 12345u128));

        let s = "cw20:mock_token:12345";
        assert_eq!(
            AssetUnchecked::from_str(s).unwrap(),
            AssetUnchecked::cw20("mock_token", 12345u128),
        );
    }

    #[test]
    fn from_sdk_string() {
        let asset = AssetUnchecked::from_sdk_string("12345uatom").unwrap();
        assert_eq!(asset, AssetUnchecked::native("uatom", 12345u128));

        let asset = AssetUnchecked::from_sdk_string(
            "69420ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2",
        )
        .unwrap();
        assert_eq!(
            asset,
            AssetUnchecked::native(
                "ibc/27394FB092D2ECCD56123C74F36E4C1F926001CEADA9CA97EA622B25F41E5EB2",
                69420u128
            ),
        );

        let asset = AssetUnchecked::from_sdk_string(
            "88888factory/osmo1z926ax906k0ycsuckele6x5hh66e2m4m6ry7dn",
        )
        .unwrap();
        assert_eq!(
            asset,
            AssetUnchecked::native(
                "factory/osmo1z926ax906k0ycsuckele6x5hh66e2m4m6ry7dn",
                88888u128
            ),
        );

        let err = AssetUnchecked::from_sdk_string("ngmi");
        assert!(err.is_err());
    }

    #[test]
    fn to_string() {
        let asset = Asset::native("uusd", 69420u128);
        assert_eq!(asset.to_string(), String::from("native:uusd:69420"));

        let asset = Asset::cw20(Addr::unchecked("mock_token"), 88888u128);
        assert_eq!(asset.to_string(), String::from("cw20:mock_token:88888"));
    }

    #[test]
    fn checking() {
        let api = MockApi::default();

        let checked = Asset::cw20(Addr::unchecked("mock_token"), 12345u128);
        let unchecked: AssetUnchecked = checked.clone().into();
        assert_eq!(unchecked.check(&api, None).unwrap(), checked);

        let checked = Asset::native("uusd", 12345u128);
        let unchecked: AssetUnchecked = checked.clone().into();
        assert_eq!(unchecked.check(&api, Some(&["uusd", "uluna", "uosmo"])).unwrap(), checked);

        let unchecked = AssetUnchecked::new(AssetInfoUnchecked::native("uatom"), 12345u128);
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

        let unchecked = AssetUnchecked::cw20("TERRA1234ABCD", 12345u128);
        assert_eq!(
            unchecked.check(&api, None).unwrap_err(),
            StdError::generic_err("Invalid input: address not normalized").into(),
        );
    }

    #[test]
    fn creating_messages() {
        let token = Asset::cw20(Addr::unchecked("mock_token"), 123456u128);
        let coin = Asset::native("uusd", 123456u128);

        let bin_msg = to_binary(&MockExecuteMsg::MockCommand {}).unwrap();
        let msg = token.send_msg("mock_contract", bin_msg.clone()).unwrap();
        assert_eq!(
            msg,
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: String::from("mock_token"),
                msg: to_binary(&Cw20ExecuteMsg::Send {
                    contract: String::from("mock_contract"),
                    amount: Uint128::new(123456),
                    msg: to_binary(&MockExecuteMsg::MockCommand {}).unwrap()
                })
                .unwrap(),
                funds: vec![]
            })
        );

        let err = coin.send_msg("mock_contract", bin_msg);
        assert_eq!(
            err,
            Err(AssetError::UnavailableMethodForNative {
                method: "send".into(),
            }),
        );

        let msg = token.transfer_msg("alice").unwrap();
        assert_eq!(
            msg,
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: String::from("mock_token"),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: String::from("alice"),
                    amount: Uint128::new(123456)
                })
                .unwrap(),
                funds: vec![]
            }),
        );

        let msg = coin.transfer_msg("alice").unwrap();
        assert_eq!(
            msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("alice"),
                amount: vec![Coin::new(123456, "uusd")]
            }),
        );

        let msg = token.transfer_from_msg("bob", "charlie").unwrap();
        assert_eq!(
            msg,
            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: String::from("mock_token"),
                msg: to_binary(&Cw20ExecuteMsg::TransferFrom {
                    owner: String::from("bob"),
                    recipient: String::from("charlie"),
                    amount: Uint128::new(123456)
                })
                .unwrap(),
                funds: vec![]
            }),
        );
        let err = coin.transfer_from_msg("bob", "charlie");
        assert_eq!(
            err,
            Err(AssetError::UnavailableMethodForNative {
                method: "transfer_from".into(),
            }),
        );
    }
}
