use std::fmt;

use cosmwasm_std::{
    to_binary, Addr, Api, BankMsg, Coin, CosmosMsg, StdError, StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "terra")]
use {cosmwasm_std::QuerierWrapper, terra_cosmwasm::TerraQuerier};

use super::asset_info::{AssetInfo, AssetInfoBase};

#[cfg(feature = "terra")]
static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetBase<T> {
    pub info: AssetInfoBase<T>,
    pub amount: Uint128,
}

pub type AssetUnchecked = AssetBase<String>;
pub type Asset = AssetBase<Addr>;

impl From<Asset> for AssetUnchecked {
    fn from(asset: Asset) -> Self {
        AssetUnchecked {
            info: asset.info.into(),
            amount: asset.amount,
        }
    }
}

impl AssetUnchecked {
    /// Validate contract address (if any) and returns a new `Asset` instance
    pub fn check(&self, api: &dyn Api) -> StdResult<Asset> {
        Ok(Asset {
            info: self.info.check(api)?,
            amount: self.amount,
        })
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.info, self.amount)
    }
}

impl Asset {
    /// Create a new `AssetBase` instance based on given asset info and amount
    pub fn new(info: AssetInfo, amount: u128) -> Self {
        Self {
            info,
            amount: amount.into(),
        }
    }

    /// Create a new `AssetBase` instance representing a CW20 token of given contract address and amount
    pub fn cw20(contract_addr: Addr, amount: u128) -> Self {
        Self {
            info: AssetInfoBase::cw20(contract_addr),
            amount: amount.into(),
        }
    }

    /// Create a new `AssetBase` instance representing a native coin of given denom
    pub fn native<A: Into<String>>(denom: A, amount: u128) -> Self {
        Self {
            info: AssetInfoBase::native(denom),
            amount: amount.into(),
        }
    }

    /// Generate a message the sends the asset from the sender to account `to`
    ///
    /// NOTE: In general, it is neccessaryto first deduct tax before calling this method.
    ///
    /// **Usage:**
    /// The following code generaates a message that sends 12345 uusd (i.e. 0.012345 UST) to Alice.
    /// Note that due to tax, the actual deliverable amount is smaller than 12345 uusd.
    ///
    /// ```rust
    /// let asset = Asset::native("uusd", 12345);
    /// let msg = Asset.deduct_tax(&deps.querier)?.transfer_msg("alice")?;
    /// ```
    pub fn transfer_msg<A: Into<String>>(&self, to: A) -> StdResult<CosmosMsg> {
        match &self.info {
            AssetInfo::Cw20(contract_addr) => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: contract_addr.into(),
                msg: to_binary(&Cw20ExecuteMsg::Transfer {
                    recipient: to.into(),
                    amount: self.amount,
                })?,
                funds: vec![],
            })),
            AssetInfo::Native(denom) => Ok(CosmosMsg::Bank(BankMsg::Send {
                to_address: to.into(),
                amount: vec![Coin {
                    denom: denom.clone(),
                    amount: self.amount,
                }],
            })),
        }
    }

    /// Generate a message that draws the asset from account `from` to account `to`
    ///
    /// **Usage:**
    /// The following code generates a message that draws 69420 uMIR token from Alice's wallet to
    /// Bob's. Note that Alice must have approve this spending for this transaction to work.
    ///
    /// ```rust
    /// let asset = Asset::cw20("mirror_token", 69420);
    /// let msg = asset.transfer_from_msg("alice", "bob")?;
    /// ```
    pub fn transfer_from_msg<A: Into<String>, B: Into<String>>(
        &self,
        from: A,
        to: B,
    ) -> StdResult<CosmosMsg> {
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
            AssetInfo::Native {
                ..
            } => Err(StdError::generic_err("native coins do not have `transfer_from` method")),
        }
    }
}

#[cfg(feature = "terra")]
impl Asset {
    /// Compute total cost (including tax) if the the asset is to be transferred
    ///
    /// **Usage:**
    /// The following code calculates to total cost for sending 100 UST. For example, if the tax
    /// that will incur from transferring 100 UST is 0.5 UST, the following code will return an
    /// `Asset` instance representing 100.5 UST.
    ///
    /// ```rust
    /// let asset = Asset::native("uusd", 100000000);
    /// let assert_with_tax = asset.add_tax(&deps.querier)?;
    /// ```
    pub fn add_tax(&self, querier: &QuerierWrapper) -> StdResult<Asset> {
        let tax = match &self.info {
            AssetInfo::Cw20(_) => Uint128::zero(),
            AssetInfo::Native(denom) => {
                if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(denom.clone())?.cap;

                    std::cmp::min(self.amount * tax_rate, tax_cap)
                }
            }
        };

        Ok(Asset {
            info: self.info.clone(),
            amount: self.amount + tax,
        })
    }

    /// Compute the deliverable amount (after tax) if the asset is to be transferred
    ///
    /// **Usage:**
    /// The following code calculates the deliverable amount if 100 UST is to be transferred. Due to
    /// tax, the deliverable amount will be smaller than the total amount.
    ///
    /// ```rust
    /// let asset = Asset::native("uusd", 100000000);
    /// let asset_after_tax = asset.deduct_tax(&deps.querier)?;
    /// ```
    pub fn deduct_tax(&self, querier: &QuerierWrapper) -> StdResult<Asset> {
        let tax = match &self.info {
            AssetInfo::Cw20(_) => Uint128::zero(),
            AssetInfo::Native(denom) => {
                if denom == "luna" {
                    Uint128::zero()
                } else {
                    let terra_querier = TerraQuerier::new(querier);
                    let tax_rate = terra_querier.query_tax_rate()?.rate;
                    let tax_cap = terra_querier.query_tax_cap(denom.clone())?.cap;

                    std::cmp::min(
                        self.amount.checked_sub(self.amount.multiply_ratio(
                            DECIMAL_FRACTION,
                            DECIMAL_FRACTION * tax_rate + DECIMAL_FRACTION,
                        ))?,
                        tax_cap,
                    )
                }
            }
        };

        Ok(Asset {
            info: self.info.clone(),
            amount: self.amount.checked_sub(tax)?,
        })
    }
}

#[cfg(feature = "legacy")]
impl From<Asset> for astroport::asset::Asset {
    fn from(asset: Asset) -> Self {
        Self {
            info: asset.info.into(),
            amount: asset.amount,
        }
    }
}

#[cfg(feature = "legacy")]
impl From<&Asset> for astroport::asset::Asset {
    fn from(asset: &Asset) -> Self {
        asset.clone().into()
    }
}

#[cfg(feature = "legacy")]
impl From<astroport::asset::Asset> for Asset {
    fn from(legacy_asset: astroport::asset::Asset) -> Self {
        Self {
            info: legacy_asset.info.into(),
            amount: legacy_asset.amount,
        }
    }
}

#[cfg(feature = "legacy")]
impl From<&astroport::asset::Asset> for Asset {
    fn from(legacy_asset: &astroport::asset::Asset) -> Self {
        legacy_asset.clone().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::MockApi;

    #[test]
    fn creating_instances() {
        let info = AssetInfo::Native(String::from("uusd"));
        let asset = Asset::new(info, 123456);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Native(String::from("uusd")),
                amount: Uint128::new(123456)
            }
        );

        let asset = Asset::cw20(Addr::unchecked("mock_token"), 123456);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("mock_token")),
                amount: Uint128::new(123456)
            }
        );

        let asset = Asset::native("uusd", 123456);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Native(String::from("uusd")),
                amount: Uint128::new(123456)
            }
        )
    }

    #[test]
    fn comparing() {
        let uluna1 = Asset::native("uluna", 69);
        let uluna2 = Asset::native("uluna", 420);
        let uusd = Asset::native("uusd", 69);
        let astro = Asset::cw20(Addr::unchecked("astro_token"), 69);

        assert_eq!(uluna1 == uluna2, false);
        assert_eq!(uluna1 == uusd, false);
        assert_eq!(astro == astro.clone(), true);
    }

    #[test]
    fn displaying() {
        let asset = Asset::native("uusd", 69420);
        assert_eq!(asset.to_string(), String::from("uusd:69420"));

        let asset = Asset::cw20(Addr::unchecked("mock_token"), 88888);
        assert_eq!(asset.to_string(), String::from("mock_token:88888"));
    }

    #[test]
    fn casting() {
        let api = MockApi::default();

        let checked = Asset::cw20(Addr::unchecked("mock_token"), 123456);
        let unchecked: AssetUnchecked = checked.clone().into();

        assert_eq!(unchecked.check(&api).unwrap(), checked);
    }

    #[test]
    fn creating_messages() {
        let asset = Asset::cw20(Addr::unchecked("mock_token"), 123456);
        let coin = Asset::native("uusd", 123456);

        let msg = asset.transfer_msg("alice").unwrap();
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
            })
        );

        let msg = coin.transfer_msg("alice").unwrap();
        assert_eq!(
            msg,
            CosmosMsg::Bank(BankMsg::Send {
                to_address: String::from("alice"),
                amount: vec![Coin::new(123456, "uusd")]
            })
        );

        let msg = asset.transfer_from_msg("bob", "charlie").unwrap();
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
            })
        );

        let err = coin.transfer_from_msg("bob", "charlie");
        assert_eq!(
            err,
            Err(StdError::generic_err("native coins do not have `transfer_from` method"))
        );
    }
}

#[cfg(all(test, feature = "terra"))]
mod tests_terra {
    use super::*;
    use crate::testing::mock_dependencies;
    use cosmwasm_std::Decimal;

    #[test]
    fn querying_balance() {
        let mut deps = mock_dependencies();
        deps.querier.set_base_balances("alice", &[Coin::new(69420, "uusd")]);
        deps.querier.set_cw20_balance("mock_token", "bob", 88888);

        let coin = AssetInfo::native("uusd");
        let balance = coin.query_balance(&deps.as_ref().querier, "alice").unwrap();
        assert_eq!(balance, Uint128::new(69420));

        let token = AssetInfo::cw20(Addr::unchecked("mock_token"));
        let balance = token.query_balance(&deps.as_ref().querier, "bob").unwrap();
        assert_eq!(balance, Uint128::new(88888));
    }

    #[test]
    fn handling_taxes() {
        let mut deps = mock_dependencies();
        deps.querier.set_native_tax_rate(Decimal::from_ratio(1u128, 1000u128)); // 0.1%
        deps.querier.set_native_tax_cap("uusd", 1000000);

        // a relatively small amount that does not hit tax cap
        let coin = Asset::native("uusd", 1234567);
        let total_amount = coin.add_tax(&deps.as_ref().querier).unwrap().amount;
        let deliverable_amount = coin.deduct_tax(&deps.as_ref().querier).unwrap().amount;
        assert_eq!(total_amount, Uint128::new(1235801));
        assert_eq!(deliverable_amount, Uint128::new(1233333));

        // a bigger amount that hits tax cap
        let coin = Asset::native("uusd", 2000000000);
        let total_amount = coin.add_tax(&deps.as_ref().querier).unwrap().amount;
        let deliverable_amount = coin.deduct_tax(&deps.as_ref().querier).unwrap().amount;
        assert_eq!(total_amount, Uint128::new(2001000000));
        assert_eq!(deliverable_amount, Uint128::new(1999000000));

        // CW20 tokens don't have the tax issue
        let coin = Asset::cw20(Addr::unchecked("mock_token"), 1234567);
        let total_amount = coin.add_tax(&deps.as_ref().querier).unwrap().amount;
        let deliverable_amount = coin.deduct_tax(&deps.as_ref().querier).unwrap().amount;
        assert_eq!(total_amount, Uint128::new(1234567));
        assert_eq!(deliverable_amount, Uint128::new(1234567));
    }
}

#[cfg(all(test, feature = "legacy"))]
mod tests_legacy {
    use super::*;

    #[test]
    fn casting_legacy() {
        let legacy_asset = astroport::asset::Asset {
            info: astroport::asset::AssetInfo::NativeToken {
                denom: String::from("uusd"),
            },
            amount: Uint128::new(69420),
        };

        let asset = Asset::native("uusd", 69420);

        assert_eq!(asset, Asset::from(&legacy_asset));
        assert_eq!(asset, Asset::from(legacy_asset.clone()));
        assert_eq!(legacy_asset, astroport::asset::Asset::from(&asset));
        assert_eq!(legacy_asset, astroport::asset::Asset::from(asset));
    }
}
