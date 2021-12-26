mod asset;
mod asset_info;

pub use asset::{Asset, AssetUnchecked};
pub use asset_info::{AssetInfo, AssetInfoUnchecked};

#[cfg(not(target_arch = "wasm32"))]
pub mod testing;
