use std::convert::TryFrom;
use std::str::FromStr;

use cosmwasm_std::{StdError, StdResult, Addr};
use cw_storage_plus::{Key, KeyDeserialize, Prefixer, PrimaryKey};

use crate::{AssetInfo, AssetInfoUnchecked};

/// TODO: add docs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetInfoKey(pub Vec<u8>);

macro_rules! impl_from {
    ($structname: ty) => {
        impl From<$structname> for AssetInfoKey {
            fn from(info: $structname) -> Self {
                Self(info.to_string().into_bytes())
            }
        }
    }
}

impl_from!(AssetInfo);
impl_from!(&AssetInfo);

impl TryFrom<AssetInfoKey> for AssetInfoUnchecked {
    type Error = StdError;

    fn try_from(key: AssetInfoKey) -> Result<Self, Self::Error> {
        let info_str = String::from_utf8(key.0)?;
        AssetInfoUnchecked::from_str(&info_str)
    }
}

impl<'a> PrimaryKey<'a> for AssetInfoKey {
    type Prefix = ();
    type SubPrefix = ();
    type Suffix = Self;
    type SuperSuffix = Self;

    fn key(&self) -> Vec<Key> {
        vec![Key::Ref(&self.0)]
    }
}

impl KeyDeserialize for AssetInfo {
    type Output = Self;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        AssetInfo::from_str(&String::from_utf8(value)?)
    }
}

impl<'a> PrimaryKey<'a> for AssetInfo {
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
            }
            AssetInfo::Cw1155(addr,id ) => {
                keys.extend("cw1155:".key());
                keys.extend(addr.key());
                keys.extend(":".key());
                keys.extend(id.key());
            }
        };
        keys
    }
}

impl<'a> Prefixer<'a> for AssetInfoKey {
    fn prefix(&self) -> Vec<Key> {
        vec![Key::Ref(&self.0)]
    }
}

impl KeyDeserialize for AssetInfoKey {
    type Output = AssetInfoUnchecked;

    #[inline(always)]
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        Self::Output::try_from(Self(value))
    }
}


impl AssetInfo {
    /// Implemented as private function to prevent from_str from being called on AssetInfo
    fn from_str(s: &str) -> Result<Self, StdError> {
        let words: Vec<&str> = s.split(':').collect();

        match words[0] {
            "native" => {
                if words.len() != 2 {
                    return Err(StdError::generic_err(
                        format!("invalid asset info format `{}`; must be in format `native:{{denom}}`", s)
                    ));
                }
                Ok(AssetInfo::Native(String::from(words[1])))
            }
            "cw20" => {
                if words.len() != 2 {
                    return Err(StdError::generic_err(
                        format!("invalid asset info format `{}`; must be in format `cw20:{{contract_addr}}`", s)
                    ));
                }
                Ok(AssetInfo::Cw20(Addr::unchecked(words[1])))
            }
            "cw1155" => {
                if words.len() != 3 {
                    return Err(StdError::generic_err(
                        format!("invalid asset info format `{}`; must be in format `cw1155:{{contract_addr}}:{{token_id}}`", s)
                    ));
                }
                Ok(AssetInfo::Cw1155(Addr::unchecked(words[1]), String::from(words[2])))
            }
            ty => Err(StdError::generic_err(
                format!("invalid asset type `{}`; must be `native` or `cw20` or `cw1155`", ty)
            )),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::mock_dependencies;
    use cosmwasm_std::{Addr, Order};
    use cw_storage_plus::Map;

    fn mock_keys() -> (AssetInfo, AssetInfo) {
        (
            AssetInfo::cw20(Addr::unchecked("mars_token")),
            AssetInfo::native("uosmo"),
        )
    }

    #[test]
    fn casting() {
        let info = AssetInfo::native("uosmo");
        let key = AssetInfoKey("native:uosmo".to_string().into_bytes());

        assert_eq!(AssetInfoKey::from(&info), key);
        assert_eq!(AssetInfoKey::from(info.clone()), key);

        assert_eq!(AssetInfoUnchecked::try_from(key).unwrap(), info.into());
    }

    #[test]
    fn storage_key_works() {
        let mut deps = mock_dependencies();
        let (key_1, key_2) = &mock_keys();
        let map: Map<AssetInfoKey, u64> = Map::new("map");

        map.save(deps.as_mut().storage, key_1.into(), &42069).unwrap();
        map.save(deps.as_mut().storage, key_2.into(), &69420).unwrap();

        assert_eq!(map.load(deps.as_ref().storage, key_1.into()).unwrap(), 42069);
        assert_eq!(map.load(deps.as_ref().storage, key_2.into()).unwrap(), 69420);

        let items = map
            .range(deps.as_ref().storage, None, None, Order::Ascending)
            .map(|item| { item.unwrap() })
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0], (key_1.into(), 42069));
        assert_eq!(items[1], (key_2.into(), 69420));
    }

    #[test]
    fn composite_key_works() {
        let mut deps = mock_dependencies();
        let (key_1, key_2) = &mock_keys();
        let map: Map<(AssetInfoKey, Addr), u64> = Map::new("map");

        map.save(
            deps.as_mut().storage,
            (key_1.into(), Addr::unchecked("larry")),
            &42069,
        )
        .unwrap();

        map.save(
            deps.as_mut().storage,
            (key_1.into(), Addr::unchecked("jake")),
            &69420,
        )
        .unwrap();

        map.save(
            deps.as_mut().storage,
            (key_2.into(), Addr::unchecked("larry")),
            &88888,
        )
        .unwrap();

        map.save(
            deps.as_mut().storage,
            (key_2.into(), Addr::unchecked("jake")),
            &123456789,
        )
        .unwrap();

        let items = map
            .prefix(key_1.into())
            .range(deps.as_ref().storage, None, None, Order::Ascending)
            .map(|item| item.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0], (Addr::unchecked("jake"), 69420));
        assert_eq!(items[1], (Addr::unchecked("larry"), 42069));

        let items = map
            .prefix(key_2.into())
            .range(deps.as_ref().storage, None, None, Order::Ascending)
            .map(|item| item.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(items.len(), 2);
        assert_eq!(items[0], (Addr::unchecked("jake"), 123456789));
        assert_eq!(items[1], (Addr::unchecked("larry"), 88888));
    }
}
