use cosmwasm_std::{OverflowError, StdError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum AssetError {
    #[error("std error encountered while handling assets: {0}")]
    Std(#[from] StdError),

    #[error("overflow error encountered while handling assets: {0}")]
    Overflow(#[from] OverflowError),

    #[error("invalid asset type `{ty}`; must be either `native` or `cw20`")]
    InvalidAssetType {
        ty: String,
    },

    #[error("invalid asset info `{received}`; must be in the format `{should_be}`")]
    InvalidAssetInfoFormat {
        /// The incorrect string that was received
        received: String,

        /// The correct string format that is expected
        should_be: String,
    },

    #[error("invalid asset `{received}`; must be in the format `native:{{denom}}:{{amount}}` or `cw20:{{contract_addr}}:{{amount}}`")]
    InvalidAssetFormat {
        received: String,
    },

    #[error("invalid asset amount `{amount}`; must be an 128-bit unsigned integer")]
    InvalidAssetAmount {
        amount: String,
    },

    #[error("failed to parse sdk coin string `{coin_str}`")]
    InvalidSdkCoin {
        coin_str: String,
    },

    #[error("denom `{denom}` is not in the whitelist; must be `{whitelist}`")]
    UnacceptedDenom {
        denom: String,
        whitelist: String,
    },

    #[error("asset `{info}` is not found in asset list")]
    NotFoundInList {
        info: String,
    },

    #[error("native coins do not have the `{method}` method")]
    UnavailableMethodForNative {
        method: String,
    },

    #[error("cannot cast asset {asset} to cosmwasm_std::Coin")]
    CannotCastToStdCoin {
        asset: String,
    },
}
