use cosmwasm_std::{
    to_binary, Addr, Api, BalanceResponse, BankMsg, BankQuery, Coin, CosmosMsg, QuerierWrapper,
    QueryRequest, StdError, StdResult, Uint128, WasmMsg, WasmQuery,
};
use cw20::{BalanceResponse as Cw20BalanceResponse, Cw20ExecuteMsg, Cw20QueryMsg};

use terra_cosmwasm::TerraQuerier;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

static DECIMAL_FRACTION: Uint128 = Uint128::new(1_000_000_000_000_000_000u128);

//--------------------------------------------------------------------------------------------------
// AssetInfo
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetInfoBase<T> {
    Cw20(T),        // the contract address, String or cosmwasm_std::Addr
    Native(String), // the native token's denom
}

impl<T> AssetInfoBase<T> {
    /// Create a new `AssetInfoBase` instance representing a CW20 token of given contract address
    pub fn cw20<A: Into<T>>(contract_addr: A) -> Self {
        Self::Cw20(contract_addr.into())
    }

    /// Create a new `AssetInfoBase` instance representing a native token of given denom
    pub fn native<A: Into<String>>(denom: A) -> Self {
        Self::Native(denom.into())
    }
}

pub type AssetInfoUnchecked = AssetInfoBase<String>;
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
    /// Validate contract address (if any) and returns a new `AssetInfo` instance
    pub fn check(&self, api: &dyn Api) -> StdResult<AssetInfo> {
        Ok(match self {
            AssetInfoUnchecked::Cw20(contract_addr) => {
                AssetInfo::Cw20(api.addr_validate(contract_addr)?)
            }
            AssetInfoUnchecked::Native(denom) => AssetInfo::Native(denom.clone()),
        })
    }
}

impl AssetInfo {
    /// Query an address' balance of the asset
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

//--------------------------------------------------------------------------------------------------
// Asset
//--------------------------------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct AssetBase<T> {
    pub info: AssetInfoBase<T>,
    pub amount: Uint128,
}

impl<T> AssetBase<T> {
    /// Create a new `AssetBase` instance based on given asset info and amount
    pub fn new<B: Into<Uint128>>(info: AssetInfoBase<T>, amount: B) -> Self {
        Self {
            info,
            amount: amount.into(),
        }
    }

    /// Create a new `AssetBase` instance representing a CW20 token of given contract address and amount
    pub fn cw20<A: Into<T>, B: Into<Uint128>>(contract_addr: A, amount: B) -> Self {
        Self {
            info: AssetInfoBase::cw20(contract_addr),
            amount: amount.into(),
        }
    }

    /// Create a new `AssetBase` instance representing a native coin of given denom
    pub fn native<A: Into<String>, B: Into<Uint128>>(denom: A, amount: B) -> Self {
        Self {
            info: AssetInfoBase::native(denom),
            amount: amount.into(),
        }
    }
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

impl Asset {
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
    pub fn transfer_msg<T: Into<String>>(&self, to: T) -> StdResult<CosmosMsg> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock_dependencies;
    use cosmwasm_std::Decimal;

    #[test]
    fn creating_instances() {
        let info = AssetInfoUnchecked::cw20("mock_token");
        assert_eq!(info, AssetInfoUnchecked::Cw20(String::from("mock_token")));

        let info = AssetInfoUnchecked::native("uusd");
        assert_eq!(info, AssetInfoUnchecked::Native(String::from("uusd")));

        let info = AssetInfo::cw20(Addr::unchecked("mock_token"));
        assert_eq!(info, AssetInfo::Cw20(Addr::unchecked("mock_token")));

        let info = AssetInfo::native("uusd");
        assert_eq!(info, AssetInfo::Native(String::from("uusd")));

        let asset = Asset::new(info, 123456 as u128);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Native(String::from("uusd")),
                amount: Uint128::new(123456)
            }
        );

        let asset = Asset::cw20(Addr::unchecked("mock_token"), 123456 as u128);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Cw20(Addr::unchecked("mock_token")),
                amount: Uint128::new(123456)
            }
        );

        let asset = Asset::native("uusd", 123456 as u128);
        assert_eq!(
            asset,
            Asset {
                info: AssetInfo::Native(String::from("uusd")),
                amount: Uint128::new(123456)
            }
        )
    }

    #[test]
    fn casting_instances() {
        let deps = mock_dependencies();

        let info_unchecked = AssetInfoUnchecked::cw20("mock_token");
        let info_checked = AssetInfo::cw20(Addr::unchecked("mock_token"));

        assert_eq!(info_unchecked.check(deps.as_ref().api).unwrap(), info_checked);
        assert_eq!(AssetInfoUnchecked::from(info_checked.clone()), info_unchecked);

        let asset_unchecked = AssetUnchecked::new(info_unchecked, 123456 as u128);
        let asset_checked = Asset::new(info_checked, 123456 as u128);

        assert_eq!(asset_unchecked.check(deps.as_ref().api).unwrap(), asset_checked);
        assert_eq!(AssetUnchecked::from(asset_checked), asset_unchecked);
    }

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
    fn creating_messages() {
        let asset = Asset::cw20(Addr::unchecked("mock_token"), 123456 as u128);
        let coin = Asset::native("uusd", 123456 as u128);

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

    #[test]
    fn handling_taxes() {
        let mut deps = mock_dependencies();
        deps.querier.set_native_tax_rate(Decimal::from_ratio(1 as u128, 1000 as u128)); // 0.1%
        deps.querier.set_native_tax_cap("uusd", 1000000);

        // a relatively small amount that does not hit tax cap
        let coin = Asset::native("uusd", 1234567 as u128);
        let total_amount = coin.add_tax(&deps.as_ref().querier).unwrap().amount;
        let deliverable_amount = coin.deduct_tax(&deps.as_ref().querier).unwrap().amount;
        assert_eq!(total_amount, Uint128::new(1235801));
        assert_eq!(deliverable_amount, Uint128::new(1233333));

        // a bigger amount that hits tax cap
        let coin = Asset::native("uusd", 2000000000 as u128);
        let total_amount = coin.add_tax(&deps.as_ref().querier).unwrap().amount;
        let deliverable_amount = coin.deduct_tax(&deps.as_ref().querier).unwrap().amount;
        assert_eq!(total_amount, Uint128::new(2001000000));
        assert_eq!(deliverable_amount, Uint128::new(1999000000));

        // CW20 tokens don't have the tax issue
        let coin = Asset::cw20(Addr::unchecked("mock_token"), 1234567 as u128);
        let total_amount = coin.add_tax(&deps.as_ref().querier).unwrap().amount;
        let deliverable_amount = coin.deduct_tax(&deps.as_ref().querier).unwrap().amount;
        assert_eq!(total_amount, Uint128::new(1234567));
        assert_eq!(deliverable_amount, Uint128::new(1234567));
    }
}
