mod asset;

pub use asset::{Asset, AssetInfo, AssetInfoUnchecked, AssetUnchecked};

#[cfg(not(target_arch = "wasm32"))]
pub mod testing;
