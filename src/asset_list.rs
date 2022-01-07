use std::fmt;

use cosmwasm_std::{Addr, Api, Coin, CosmosMsg, StdError, StdResult};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::asset::{Asset, AssetBase};
use super::asset_info::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetListBase<T>(Vec<AssetBase<T>>);

#[allow(clippy::derivable_impls)] // clippy says `Default` can be derived here, but actually it can't
impl<T> Default for AssetListBase<T> {
    fn default() -> Self {
        Self(vec![])
    }
}

pub type AssetListUnchecked = AssetListBase<String>;
pub type AssetList = AssetListBase<Addr>;

impl From<AssetList> for AssetListUnchecked {
    fn from(list: AssetList) -> Self {
        Self(list.to_vec().iter().cloned().map(|asset| asset.into()).collect())
    }
}

impl AssetListUnchecked {
    /// Validate contract address of every asset in the list, and return a new `AssetList` instance
    pub fn check(&self, api: &dyn Api) -> StdResult<AssetList> {
        Ok(AssetList::from(
            self.0.iter().map(|asset| asset.check(api)).collect::<StdResult<Vec<Asset>>>()?,
        ))
    }
}

impl fmt::Display for AssetList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0.iter().map(|asset| asset.to_string()).collect::<Vec<String>>().join(",")
        )
    }
}

impl From<Vec<Asset>> for AssetList {
    fn from(vec: Vec<Asset>) -> Self {
        Self(vec)
    }
}

impl From<&[Asset]> for AssetList {
    fn from(vec: &[Asset]) -> Self {
        vec.to_vec().into()
    }
}

impl From<Vec<Coin>> for AssetList {
    fn from(coins: Vec<Coin>) -> Self {
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
    pub fn new() -> Self {
        AssetListBase::default()
    }

    /// Return a copy of the underlying vector
    pub fn to_vec(&self) -> Vec<Asset> {
        self.0.clone()
    }

    /// Return length of the asset list
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Find an asset in the list that matches the provided asset info
    ///
    /// Return `Some(&asset)` if found, where `&asset` is a reference to the asset found; `None` if
    /// not found.
    pub fn find(&self, info: &AssetInfo) -> Option<&Asset> {
        self.0.iter().find(|asset| asset.info == *info)
    }

    /// Apply a mutation on each of the asset
    pub fn apply<F: FnMut(&mut Asset)>(&mut self, f: F) -> &mut Self {
        self.0.iter_mut().for_each(f);
        self
    }

    /// Removes all assets in the list that has zero amount
    pub fn purge(&mut self) -> &mut Self {
        self.0.retain(|asset| !asset.amount.is_zero());
        self
    }

    /// Add a new asset to the list
    ///
    /// If asset of the same kind already exists in the list, then increment its amount; if not,
    /// append to the end of the list.
    pub fn add(&mut self, asset_to_add: &Asset) -> StdResult<&mut Self> {
        match self.0.iter_mut().find(|asset| asset.info == asset_to_add.info) {
            Some(asset) => {
                asset.amount = asset.amount.checked_add(asset_to_add.amount)?;
            }
            None => {
                self.0.push(asset_to_add.clone());
            }
        }
        Ok(self.purge())
    }

    /// Add multiple new assets to the list
    pub fn add_many(&mut self, assets_to_add: &AssetList) -> StdResult<&mut Self> {
        for asset in &assets_to_add.0 {
            self.add(asset)?;
        }
        Ok(self)
    }

    /// Deduct an asset from the list
    ///
    /// The asset of the same kind and equal or greater amount must already exist in the list. If so,
    /// deduct the amount from the asset; ifnot, throw an error.
    ///
    /// If an asset's amount is reduced to zero, it is purged from the list.
    pub fn deduct(&mut self, asset_to_deduct: &Asset) -> StdResult<&mut Self> {
        match self.0.iter_mut().find(|asset| asset.info == asset_to_deduct.info) {
            Some(asset) => {
                asset.amount = asset.amount.checked_sub(asset_to_deduct.amount)?;
            }
            None => {
                return Err(StdError::generic_err(
                    format!("not found in asset list: {}", asset_to_deduct.info)
                ));
            }
        }
        Ok(self.purge())
    }

    /// Deduct multiple assets from the list
    pub fn deduct_many(&mut self, assets_to_deduct: &AssetList) -> StdResult<&mut Self> {
        for asset in &assets_to_deduct.0 {
            self.deduct(asset)?;
        }
        Ok(self)
    }

    /// Generate a transfer messages for every asset in the list
    pub fn transfer_msgs<A: Into<String> + Clone>(&self, to: A) -> StdResult<Vec<CosmosMsg>> {
        self.0
            .iter()
            .map(|asset| asset.transfer_msg(to.clone()))
            .collect::<StdResult<Vec<CosmosMsg>>>()
    }
}

#[cfg(feature = "terra")]
impl AssetList {
    /// Execute `add_tax` to every asset in the list
    pub fn add_tax(&mut self, querier: &QuerierWrapper) -> StdResult<&mut Self> {
        for asset in &mut self.0 {
            asset.add_tax(querier)?;
        }
        Ok(self)
    }

    /// Execute `deduct_tax` to every asset in the list
    pub fn deduct_tax(&mut self, querier: &QuerierWrapper) -> StdResult<&mut Self> {
        for asset in &mut self.0 {
            asset.deduct_tax(querier)?;
        }
        Ok(self)
    }
}

#[cfg(feature = "legacy")]
impl From<AssetList> for Vec<astroport::asset::Asset> {
    fn from(list: AssetList) -> Self {
        list.0.iter().map(|asset| asset.into()).collect()
    }
}

#[cfg(feature = "legacy")]
impl From<&AssetList> for Vec<astroport::asset::Asset> {
    fn from(list: &AssetList) -> Self {
        list.clone().into()
    }
}

#[cfg(feature = "legacy")]
impl AssetList {
    pub fn from_legacy(legacy_list: &[astroport::asset::Asset]) -> Self {
        Self(legacy_list.to_vec().iter().map(|asset| asset.into()).collect())
    }
}

#[cfg(test)]
mod test_helpers {
    use super::super::asset::Asset;
    use super::*;

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
        AssetList::from(vec![Asset::native("uusd", 69420u128), Asset::new(mock_token(), 88888u128)])
    }
}

#[cfg(test)]
mod tests {
    use super::super::asset::Asset;
    use super::test_helpers::{mock_list, mock_token, uluna, uusd};
    use super::*;
    use cosmwasm_std::testing::MockApi;
    use cosmwasm_std::{
        to_binary, BankMsg, Coin, CosmosMsg, Decimal, OverflowError, OverflowOperation, Uint128,
        WasmMsg,
    };
    use cw20::Cw20ExecuteMsg;

    #[test]
    fn displaying() {
        let list = mock_list();
        assert_eq!(list.to_string(), String::from("uusd:69420,mock_token:88888"));
    }

    #[test]
    fn casting() {
        let api = MockApi::default();

        let checked = mock_list();
        let unchecked: AssetListUnchecked = checked.clone().into();

        assert_eq!(unchecked.check(&api).unwrap(), checked);
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
            ])
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

        let expected = mock_list().apply(|a| a.amount = a.amount * Uint128::new(2)).clone();
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
        assert_eq!(err, Err(StdError::generic_err("not found in asset list: uusd")));

        list.deduct(&Asset::new(mock_token(), 12345u128)).unwrap();
        let asset = list.find(&mock_token()).unwrap();
        assert_eq!(asset.amount, Uint128::new(76543));

        let err = list.deduct(&Asset::new(mock_token(), 99999u128));
        assert_eq!(
            err,
            Err(StdError::overflow(OverflowError::new(
                OverflowOperation::Sub,
                Uint128::new(76543),
                Uint128::new(99999)
            )))
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
                })
            ]
        );
    }
}

#[cfg(all(test, feature = "legacy"))]
mod tests_legacy {
    use super::test_helpers::mock_list;
    use super::*;
    use cosmwasm_std::Uint128;

    #[test]
    fn casting_legacy() {
        let legacy_list = vec![
            astroport::asset::Asset {
                info: astroport::asset::AssetInfo::NativeToken {
                    denom: String::from("uusd"),
                },
                amount: Uint128::new(69420),
            },
            astroport::asset::Asset {
                info: astroport::asset::AssetInfo::Token {
                    contract_addr: Addr::unchecked("mock_token"),
                },
                amount: Uint128::new(88888),
            },
        ];

        let list = mock_list();

        assert_eq!(list, AssetList::from_legacy(&legacy_list));
        assert_eq!(legacy_list, Vec::<astroport::asset::Asset>::from(&list));
        assert_eq!(legacy_list, Vec::<astroport::asset::Asset>::from(list));
    }
}
