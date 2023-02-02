use std::{fmt, str::FromStr};

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, Coin, CosmosMsg};
use cw_address_like::AddressLike;

use crate::{Asset, AssetBase, AssetError, AssetInfo, AssetUnchecked};

/// Represents a list of fungible tokens, each with a known amount
#[cw_serde]
pub struct AssetListBase<T: AddressLike>(Vec<AssetBase<T>>);

#[allow(clippy::derivable_impls)] // clippy says `Default` can be derived here, but actually it can't
impl<T: AddressLike> Default for AssetListBase<T> {
    fn default() -> Self {
        Self(vec![])
    }
}

/// Represents an **asset list** instance that may contain unverified data; to
/// be used in messages.
pub type AssetListUnchecked = AssetListBase<String>;

/// Represents an **asset list** instance containing only verified data; to be
/// used in contract storage.
pub type AssetList = AssetListBase<Addr>;

impl FromStr for AssetListUnchecked {
    type Err = AssetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(Self(vec![]));
        }

        s
            .split(',')
            .map(AssetUnchecked::from_str)
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

impl From<AssetList> for AssetListUnchecked {
    fn from(list: AssetList) -> Self {
        Self(list.to_vec().iter().cloned().map(|asset| asset.into()).collect())
    }
}

impl AssetListUnchecked {
    /// Validate data contained in an _unchecked_ **asset list** instance,
    /// return a new _checked_ **asset list** instance:
    ///
    /// - For CW20 tokens, assert the contract address is valid
    /// - For SDK coins, assert that the denom is included in a given whitelist; skip if the
    ///   whitelist is not provided
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Api, StdResult};
    /// use cw_asset::{Asset, AssetList, AssetListUnchecked, AssetUnchecked};
    ///
    /// fn validate_assets(api: &dyn Api, list_unchecked: &AssetListUnchecked) {
    ///     match list_unchecked.check(api, Some(&["uatom", "uluna"])) {
    ///         Ok(list) => println!("asset list is valid: {}", list.to_string()),
    ///         Err(err) => println!("asset list is invalid! reason: {}", err),
    ///     }
    /// }
    /// ```
    pub fn check(
        &self,
        api: &dyn Api,
        optional_whitelist: Option<&[&str]>,
    ) -> Result<AssetList, AssetError> {
        self.0
            .iter()
            .map(|asset| asset.check(api, optional_whitelist))
            .collect::<Result<Vec<_>, _>>()
            .map(AssetList::from)
    }
}

impl fmt::Display for AssetList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = if self.is_empty() {
            "[]".to_string()
        } else {
            self.0
                .iter()
                .map(|asset| asset.to_string())
                .collect::<Vec<_>>()
                .join(",")
        };

        write!(f, "{s}")
    }
}

impl std::ops::Index<usize> for AssetList {
    type Output = Asset;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::Index<usize> for &AssetList {
    type Output = Asset;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<'a> IntoIterator for &'a AssetList {
    type Item = &'a Asset;
    type IntoIter = std::slice::Iter<'a, Asset>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl From<Vec<Asset>> for AssetList {
    fn from(vec: Vec<Asset>) -> Self {
        Self(vec)
    }
}

impl From<&Vec<Asset>> for AssetList {
    fn from(vec: &Vec<Asset>) -> Self {
        Self(vec.clone())
    }
}

impl From<&[Asset]> for AssetList {
    fn from(vec: &[Asset]) -> Self {
        vec.to_vec().into()
    }
}

impl From<Vec<Coin>> for AssetList {
    fn from(coins: Vec<Coin>) -> Self {
        (&coins).into()
    }
}

impl From<&Vec<Coin>> for AssetList {
    fn from(coins: &Vec<Coin>) -> Self {
        Self(coins.iter().map(|coin| coin.into()).collect())
    }
}

impl From<&[Coin]> for AssetList {
    fn from(coins: &[Coin]) -> Self {
        coins.to_vec().into()
    }
}

impl AssetList {
    /// Create a new, empty asset list
    ///
    /// ```rust
    /// use cw_asset::AssetList;
    ///
    /// let list = AssetList::new();
    /// let len = list.len(); // should be zero
    /// ```
    pub fn new() -> Self {
        AssetListBase::default()
    }

    /// Return a copy of the underlying vector
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetList};
    ///
    /// let list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    ///     Asset::native("uusd", 67890u128),
    /// ]);
    ///
    /// let vec: Vec<Asset> = list.to_vec();
    /// ```
    pub fn to_vec(&self) -> Vec<Asset> {
        self.0.clone()
    }

    /// Return length of the asset list
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetList};
    ///
    /// let list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    ///     Asset::native("uusd", 67890u128),
    /// ]);
    ///
    /// let len = list.len(); // should be two
    /// ```
    ///
    // NOTE: I do have `is_empty` implemented, but clippy still throws a warnin
    // saying I don't have it. Must be a clippy bug...
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return whether the asset list is empty
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetList};
    ///
    /// let mut list = AssetList::from(vec![Asset::native("uluna", 12345u128)]);
    /// let is_empty = list.is_empty(); // should be `false`
    ///
    /// list.deduct(&Asset::native("uluna", 12345u128)).unwrap();
    /// let is_empty = list.is_empty(); // should be `true`
    /// ```
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Find an asset in the list that matches the provided asset info
    ///
    /// Return `Some(&asset)` if found, where `&asset` is a reference to the
    /// asset found; `None` if not found.
    ///
    /// A case where is method is useful is to find how much asset the user sent
    /// along with a message:
    ///
    /// ```rust
    /// use cosmwasm_std::MessageInfo;
    /// use cw_asset::{AssetInfo, AssetList};
    ///
    /// fn find_uusd_received_amount(info: &MessageInfo) {
    ///     let list = AssetList::from(&info.funds);
    ///     match list.find(&AssetInfo::native("uusd")) {
    ///         Some(asset) => println!("received {} uusd", asset.amount),
    ///         None => println!("did not receive any uusd"),
    ///     }
    /// }
    /// ```
    pub fn find(&self, info: &AssetInfo) -> Option<&Asset> {
        self.0.iter().find(|asset| asset.info == *info)
    }

    /// Apply a mutation on each of the asset
    ///
    /// An example case where this is useful is to scale the amount of each
    /// asset in the list by a certain factor:
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetInfo, AssetList};
    ///
    /// let mut list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    ///     Asset::native("uusd", 67890u128),
    /// ]);
    ///
    /// let list_halved = list.apply(|a| a.amount = a.amount.multiply_ratio(1u128, 2u128));
    /// ```
    pub fn apply<F: FnMut(&mut Asset)>(&mut self, f: F) -> &mut Self {
        self.0.iter_mut().for_each(f);
        self
    }

    /// Removes all assets in the list that has zero amount
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetList};
    ///
    /// let mut list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    ///     Asset::native("uusd", 0u128),
    /// ]);
    /// let mut len = list.len(); // should be two
    ///
    /// list.purge();
    /// len = list.len(); // should be one
    /// ```
    pub fn purge(&mut self) -> &mut Self {
        self.0.retain(|asset| !asset.amount.is_zero());
        self
    }

    /// Add a new asset to the list
    ///
    /// If asset of the same kind already exists in the list, then increment its
    /// amount; if not, append to the end of the list.
    ///
    /// NOTE: `purge` is automatically performed following the addition, so
    /// adding an asset with zero amount has no effect.
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetInfo, AssetList};
    ///
    /// let mut list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    /// ]);
    ///
    /// list.add(&Asset::native("uusd", 67890u128));
    /// let mut len = list.len();  // should be two
    ///
    /// list.add(&Asset::native("uluna", 11111u128));
    /// len = list.len();  // should still be two
    ///
    /// let uluna_amount = list
    ///     .find(&AssetInfo::native("uluna"))
    ///     .unwrap()
    ///     .amount;  // should have increased to 23456
    /// ```
    pub fn add(&mut self, asset_to_add: &Asset) -> Result<&mut Self, AssetError> {
        match self.0.iter_mut().find(|asset| asset.info == asset_to_add.info) {
            Some(asset) => {
                asset.amount = asset.amount.checked_add(asset_to_add.amount)?;
            },
            None => {
                self.0.push(asset_to_add.clone());
            },
        }
        Ok(self.purge())
    }

    /// Add multiple new assets to the list
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetInfo, AssetList};
    ///
    /// let mut list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    /// ]);
    ///
    /// list.add_many(&AssetList::from(vec![
    ///     Asset::native("uusd", 67890u128),
    ///     Asset::native("uluna", 11111u128),
    /// ]));
    ///
    /// let uusd_amount = list
    ///     .find(&AssetInfo::native("uusd"))
    ///     .unwrap()
    ///     .amount;  // should be 67890
    ///
    /// let uluna_amount = list
    ///     .find(&AssetInfo::native("uluna"))
    ///     .unwrap()
    ///     .amount;  // should have increased to 23456
    /// ```
    pub fn add_many(&mut self, assets_to_add: &AssetList) -> Result<&mut Self, AssetError> {
        for asset in &assets_to_add.0 {
            self.add(asset)?;
        }
        Ok(self)
    }

    /// Deduct an asset from the list
    ///
    /// The asset of the same kind and equal or greater amount must already
    /// exist in the list. If so, deduct the amount from the asset; ifnot, throw
    /// an error.
    ///
    /// NOTE: `purge` is automatically performed following the addition.
    /// Therefore, if an asset's amount is reduced to zero, it will be removed
    /// from the list.
    ///
    /// ```
    /// use cw_asset::{Asset, AssetInfo, AssetList};
    ///
    /// let mut list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    /// ]);
    ///
    /// list.deduct(&Asset::native("uluna", 10000u128)).unwrap();
    ///
    /// let uluna_amount = list
    ///     .find(&AssetInfo::native("uluna"))
    ///     .unwrap()
    ///     .amount;  // should have reduced to 2345
    ///
    /// list.deduct(&Asset::native("uluna", 2345u128));
    ///
    /// let len = list.len();  // should be zero, as uluna is purged from the list
    /// ```
    pub fn deduct(&mut self, asset_to_deduct: &Asset) -> Result<&mut Self, AssetError> {
        match self.0.iter_mut().find(|asset| asset.info == asset_to_deduct.info) {
            Some(asset) => {
                asset.amount = asset.amount.checked_sub(asset_to_deduct.amount)?;
            },
            None => {
                return Err(AssetError::NotFoundInList {
                    info: asset_to_deduct.info.to_string(),
                });
            },
        }
        Ok(self.purge())
    }

    /// Deduct multiple assets from the list
    ///
    /// ```rust
    /// use cw_asset::{Asset, AssetInfo, AssetList};
    ///
    /// let mut list = AssetList::from(vec![
    ///     Asset::native("uluna", 12345u128),
    ///     Asset::native("uusd", 67890u128),
    /// ]);
    ///
    /// list.deduct_many(&AssetList::from(vec![
    ///     Asset::native("uluna", 2345u128),
    ///     Asset::native("uusd", 67890u128),
    /// ])).unwrap();
    ///
    /// let uluna_amount = list
    ///     .find(&AssetInfo::native("uluna"))
    ///     .unwrap()
    ///     .amount;  // should have reduced to 2345
    ///
    /// let len = list.len();  // should be zero, as uusd is purged from the list
    /// ```
    pub fn deduct_many(&mut self, assets_to_deduct: &AssetList) -> Result<&mut Self, AssetError> {
        for asset in &assets_to_deduct.0 {
            self.deduct(asset)?;
        }
        Ok(self)
    }

    /// Generate a transfer messages for every asset in the list
    ///
    /// ```rust
    /// use cosmwasm_std::{Addr, Response};
    /// use cw_asset::{AssetError, AssetList};
    ///
    /// fn transfer_assets(
    ///     list: &AssetList,
    ///     recipient_addr: &Addr,
    /// ) -> Result<Response, AssetError> {
    ///     let msgs = list.transfer_msgs(recipient_addr)?;
    ///
    ///     Ok(Response::new()
    ///         .add_messages(msgs)
    ///         .add_attribute("assets_sent", list.to_string()))
    /// }
    /// ```
    pub fn transfer_msgs<A: Into<String> + Clone>(
        &self,
        to: A,
    ) -> Result<Vec<CosmosMsg>, AssetError> {
        self.0
            .iter()
            .map(|asset| asset.transfer_msg(to.clone()))
            .collect()
    }
}

//------------------------------------------------------------------------------
// Tests
//------------------------------------------------------------------------------

#[cfg(test)]
mod test_helpers {
    use super::*;
    use crate::Asset;

    pub fn uluna() -> AssetInfo {
        AssetInfo::native("uluna")
    }

    pub fn uusd() -> AssetInfo {
        AssetInfo::native("uusd")
    }

    pub fn mock_token() -> AssetInfo {
        AssetInfo::cw20(Addr::unchecked("mock_token"))
    }

    pub fn mock_list() -> AssetList {
        AssetList::from(vec![
            Asset::native("uusd", 69420u128),
            Asset::new(mock_token(), 88888u128),
        ])
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        testing::MockApi, to_binary, BankMsg, Coin, CosmosMsg, Decimal, OverflowError,
        OverflowOperation, StdError, Uint128, WasmMsg,
    };
    use cw20::Cw20ExecuteMsg;

    use super::{
        super::asset::{Asset, AssetUnchecked},
        test_helpers::{mock_list, mock_token, uluna, uusd},
        *,
    };

    #[test]
    fn from_string() {
        let s = "";
        assert_eq!(AssetListUnchecked::from_str(s).unwrap(), AssetListBase::<String>(vec![]));

        let s = "native:uusd:69420,cw20:mock_token";
        assert_eq!(
            AssetListUnchecked::from_str(s),
            Err(AssetError::InvalidAssetFormat {
                received: "cw20:mock_token".into(),
            }),
        );

        let s = "native:uusd:69420,cw721:galactic_punk:1";
        assert_eq!(
            AssetListUnchecked::from_str(s),
            Err(AssetError::InvalidAssetType {
                ty: "cw721".into(),
            }),
        );

        let s = "native:uusd:69420,cw20:mock_token:ngmi";
        assert_eq!(
            AssetListUnchecked::from_str(s),
            Err(AssetError::InvalidAssetAmount {
                amount: "ngmi".into(),
            }),
        );

        let s = "native:uusd:69420,cw20:mock_token:88888";
        assert_eq!(AssetListUnchecked::from_str(s).unwrap(), AssetListUnchecked::from(mock_list()));
    }

    #[test]
    fn to_string() {
        let list = mock_list();
        assert_eq!(list.to_string(), String::from("native:uusd:69420,cw20:mock_token:88888"));

        let list = AssetList::from(vec![] as Vec<Asset>);
        assert_eq!(list.to_string(), String::from("[]"));
    }

    #[test]
    fn indexing() {
        let list = mock_list();
        let vec = list.to_vec();
        assert_eq!(list[0], vec[0]);
        assert_eq!(list[1], vec[1]);
    }

    #[test]
    fn iterating() {
        let list = mock_list();

        let strs: Vec<String> = list.into_iter().map(|asset| asset.to_string()).collect();
        assert_eq!(
            strs,
            vec![String::from("native:uusd:69420"), String::from("cw20:mock_token:88888"),]
        );
    }

    #[test]
    fn checking() {
        let api = MockApi::default();

        let checked = mock_list();
        let unchecked: AssetListUnchecked = checked.clone().into();
        assert_eq!(unchecked.check(&api, None).unwrap(), checked);
        assert_eq!(unchecked.check(&api, Some(&["uusd", "uluna"])).unwrap(), checked);
        assert_eq!(
            unchecked.check(&api, Some(&["uatom", "uosmo", "uscrt"])),
            Err(AssetError::UnacceptedDenom {
                denom: "uusd".into(),
                whitelist: "uatom|uosmo|uscrt".into(),
            }),
        );
    }

    #[test]
    fn checking_uppercase() {
        let api = MockApi::default();

        let unchecked = AssetListBase(vec![
            AssetUnchecked::native("uusd", 69420u128),
            AssetUnchecked::cw20("MOCK_TOKEN", 88888u128),
        ]);

        assert_eq!(
            unchecked.check(&api, None).unwrap_err(),
            StdError::generic_err("Invalid input: address not normalized").into(),
        );
    }

    #[test]
    fn finding() {
        let list = mock_list();

        let asset_option = list.find(&uusd());
        assert_eq!(asset_option, Some(&Asset::new(uusd(), 69420u128)));

        let asset_option = list.find(&mock_token());
        assert_eq!(asset_option, Some(&Asset::new(mock_token(), 88888u128)));
    }

    #[test]
    fn applying() {
        let mut list = mock_list();

        let half = Decimal::from_ratio(1u128, 2u128);
        list.apply(|asset: &mut Asset| asset.amount = asset.amount * half);
        assert_eq!(
            list,
            AssetList::from(vec![
                Asset::native("uusd", 34710u128),
                Asset::new(mock_token(), 44444u128)
            ]),
        );
    }

    #[test]
    fn adding() {
        let mut list = mock_list();

        list.add(&Asset::new(uluna(), 12345u128)).unwrap();
        let asset = list.find(&uluna()).unwrap();
        assert_eq!(asset.amount, Uint128::new(12345));

        list.add(&Asset::new(uusd(), 1u128)).unwrap();
        let asset = list.find(&uusd()).unwrap();
        assert_eq!(asset.amount, Uint128::new(69421));
    }

    #[test]
    fn adding_many() {
        let mut list = mock_list();
        list.add_many(&mock_list()).unwrap();

        let expected = mock_list().apply(|a| a.amount *= Uint128::new(2)).clone();
        assert_eq!(list, expected);
    }

    #[test]
    fn deducting() {
        let mut list = mock_list();

        list.deduct(&Asset::new(uusd(), 12345u128)).unwrap();
        let asset = list.find(&uusd()).unwrap();
        assert_eq!(asset.amount, Uint128::new(57075));

        list.deduct(&Asset::new(uusd(), 57075u128)).unwrap();
        let asset_option = list.find(&uusd());
        assert_eq!(asset_option, None);

        let err = list.deduct(&Asset::new(uusd(), 57075u128));
        assert_eq!(
            err,
            Err(AssetError::NotFoundInList {
                info: "native:uusd".into(),
            }),
        );

        list.deduct(&Asset::new(mock_token(), 12345u128)).unwrap();
        let asset = list.find(&mock_token()).unwrap();
        assert_eq!(asset.amount, Uint128::new(76543));

        let err = list.deduct(&Asset::new(mock_token(), 99999u128));
        assert_eq!(
            err,
            Err(OverflowError::new(
                OverflowOperation::Sub,
                Uint128::new(76543),
                Uint128::new(99999)
            )
            .into()),
        );
    }

    #[test]
    fn deducting_many() {
        let mut list = mock_list();
        list.deduct_many(&mock_list()).unwrap();
        assert_eq!(list, AssetList::new());
    }

    #[test]
    fn creating_messages() {
        let list = mock_list();
        let msgs = list.transfer_msgs("alice").unwrap();
        assert_eq!(
            msgs,
            vec![
                CosmosMsg::Bank(BankMsg::Send {
                    to_address: String::from("alice"),
                    amount: vec![Coin::new(69420, "uusd")]
                }),
                CosmosMsg::Wasm(WasmMsg::Execute {
                    contract_addr: String::from("mock_token"),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: String::from("alice"),
                        amount: Uint128::new(88888)
                    })
                    .unwrap(),
                    funds: vec![]
                }),
            ],
        );
    }
}
