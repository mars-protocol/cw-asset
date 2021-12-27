use std::fmt;

use cosmwasm_std::{Addr, Api, CosmosMsg, QuerierWrapper, StdError, StdResult};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::asset::{Asset, AssetBase};
use super::asset_info::AssetInfo;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetListBase<T>(Vec<AssetBase<T>>);

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

#[allow(clippy::derivable_impls)] // clippy says `Default` can be derived here, but actually it can't
impl Default for AssetList {
    fn default() -> Self {
        Self(vec![])
    }
}

impl From<Vec<Asset>> for AssetList {
    fn from(vec: Vec<Asset>) -> Self {
        Self(vec)
    }
}

impl AssetList {
    /// Create a new, empty asset list
    pub fn new() -> Self {
        AssetListBase::default()
    }

    /// Returns the vector of assets
    pub fn to_vec(&self) -> Vec<Asset> {
        self.0.clone()
    }

    /// Find an asset in the list that matches the provided asset info
    ///
    /// Return `Some(&asset)` if found, where `&asset` is a reference to the asset found; `None` if
    /// not found.
    pub fn find(&self, info: &AssetInfo) -> Option<&Asset> {
        self.0.iter().find(|asset| asset.info == *info)
    }

    /// Add a new asset to the list
    ///
    /// If asset of the same kind already exists in the list, then increment its amount; if not,
    /// append to the end of the list.
    pub fn add(&mut self, asset_to_add: &Asset) -> StdResult<()> {
        match self.0.iter_mut().find(|asset| asset.info == asset_to_add.info) {
            Some(asset) => {
                asset.amount = asset.amount.checked_add(asset_to_add.amount)?;
            }
            None => {
                self.0.push(asset_to_add.clone());
            }
        }

        Ok(())
    }

    /// Deduct an asset from the list
    ///
    /// The asset of the same kind and equal or greater amount must already exist in the list. If so,
    /// deduct the amount from the asset; ifnot, throw an error.
    ///
    /// If an asset's amount is reduced to zero, it is purged from the list.
    pub fn deduct(&mut self, asset_to_deduct: &Asset) -> StdResult<()> {
        match self.0.iter_mut().find(|asset| asset.info == asset_to_deduct.info) {
            Some(asset) => {
                asset.amount = asset.amount.checked_sub(asset_to_deduct.amount)?;
            }
            None => {
                return Err(StdError::generic_err(format!("not found: {}", asset_to_deduct.info)))
            }
        }

        self.0.retain(|asset| !asset.amount.is_zero());

        Ok(())
    }

    /// Execute `add_tax` to every asset in the list; returns a new `AssetList` instance with the
    /// updated amounts
    pub fn add_tax(&self, querier: &QuerierWrapper) -> StdResult<AssetList> {
        Ok(Self(
            self.0.iter().map(|asset| asset.add_tax(querier)).collect::<StdResult<Vec<Asset>>>()?,
        ))
    }

    /// Execute `deduct_tax` to every asset in the list; returns a new `AssetList` instance with the
    /// updated amounts
    pub fn deduct_tax(&self, querier: &QuerierWrapper) -> StdResult<AssetList> {
        Ok(Self(
            self.0
                .iter()
                .map(|asset| asset.deduct_tax(querier))
                .collect::<StdResult<Vec<Asset>>>()?,
        ))
    }

    /// Generate a transfer messages for every asset in the list
    pub fn transfer_msgs<A: Into<String> + Clone>(&self, to: A) -> StdResult<Vec<CosmosMsg>> {
        self.0
            .iter()
            .map(|asset| asset.transfer_msg(to.clone()))
            .collect::<StdResult<Vec<CosmosMsg>>>()
    }
}

#[cfg(test)]
mod tests {
    use super::super::asset::Asset;
    use super::super::asset_info::AssetInfo;
    use super::*;
    use crate::testing::mock_dependencies;
    use cosmwasm_std::{
        to_binary, BankMsg, Coin, CosmosMsg, Decimal, OverflowError, OverflowOperation, Uint128,
        WasmMsg,
    };
    use cw20::Cw20ExecuteMsg;

    fn uusd() -> AssetInfo {
        AssetInfo::native("uusd")
    }

    fn mock_token() -> AssetInfo {
        AssetInfo::cw20(Addr::unchecked("mock_token"))
    }

    fn mock_list() -> AssetList {
        AssetList::from(vec![Asset::new(uusd(), 69420), Asset::new(mock_token(), 88888)])
    }

    #[test]
    fn displaying() {
        let list = mock_list();
        assert_eq!(list.to_string(), String::from("uusd:69420,mock_token:88888"));
    }

    #[test]
    fn casting() {
        let deps = mock_dependencies();

        let checked = mock_list();
        let unchecked: AssetListUnchecked = checked.clone().into();

        assert_eq!(unchecked.check(deps.as_ref().api).unwrap(), checked);
    }

    #[test]
    fn finding_asset() {
        let list = mock_list();

        let asset_option = list.find(&uusd());
        assert_eq!(asset_option, Some(&Asset::new(uusd(), 69420)));

        let asset_option = list.find(&mock_token());
        assert_eq!(asset_option, Some(&Asset::new(mock_token(), 88888)));
    }

    #[test]
    fn adding_asset() {
        let mut list = AssetList::new();

        list.add(&Asset::new(uusd(), 69420)).unwrap();
        list.add(&Asset::new(mock_token(), 88888)).unwrap();
        assert_eq!(list, mock_list());

        list.add(&Asset::new(uusd(), 1)).unwrap();
        let asset = list.find(&uusd()).unwrap();
        assert_eq!(asset.amount.u128(), 69421);
    }

    #[test]
    fn deducting_asset() {
        let mut list = mock_list();

        list.deduct(&Asset::new(uusd(), 12345)).unwrap();
        let asset = list.find(&uusd()).unwrap();
        assert_eq!(asset.amount.u128(), 57075);

        list.deduct(&Asset::new(uusd(), 57075)).unwrap();
        let asset_option = list.find(&uusd());
        assert_eq!(asset_option, None);

        let err = list.deduct(&Asset::new(uusd(), 57075));
        assert_eq!(err, Err(StdError::generic_err("not found: uusd")));

        list.deduct(&Asset::new(mock_token(), 12345)).unwrap();
        let asset = list.find(&mock_token()).unwrap();
        assert_eq!(asset.amount.u128(), 76543);

        let err = list.deduct(&Asset::new(mock_token(), 99999));
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

    #[test]
    fn handling_taxes() {
        let mut deps = mock_dependencies();
        deps.querier.set_native_tax_rate(Decimal::from_ratio(1u128, 1000u128)); // 0.1%
        deps.querier.set_native_tax_cap("uusd", 1000000);

        let list = mock_list();

        let list_with_tax = list.add_tax(&deps.as_ref().querier).unwrap();
        assert_eq!(
            list_with_tax,
            AssetList::from(vec![Asset::new(uusd(), 69489), Asset::new(mock_token(), 88888)])
        );

        let list_after_tax = list.deduct_tax(&deps.as_ref().querier).unwrap();
        assert_eq!(
            list_after_tax,
            AssetList::from(vec![Asset::new(uusd(), 69350), Asset::new(mock_token(), 88888)])
        );
    }
}
