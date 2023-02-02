use std::{convert::TryFrom, str::FromStr};

use cosmwasm_std::{StdError, StdResult};
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
    };
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

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use cosmwasm_std::{testing::mock_dependencies, Addr, Order};
    use cw_storage_plus::Map;

    use super::*;

    fn mock_keys() -> (AssetInfo, AssetInfo) {
        (AssetInfo::cw20(Addr::unchecked("mars_token")), AssetInfo::native("uosmo"))
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
            .map(|item| item.unwrap())
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

        map.save(deps.as_mut().storage, (key_1.into(), Addr::unchecked("larry")), &42069).unwrap();

        map.save(deps.as_mut().storage, (key_1.into(), Addr::unchecked("jake")), &69420).unwrap();

        map.save(deps.as_mut().storage, (key_2.into(), Addr::unchecked("larry")), &88888).unwrap();

        map.save(deps.as_mut().storage, (key_2.into(), Addr::unchecked("jake")), &123456789)
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
