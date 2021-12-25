use cosmwasm_std::testing::{MockApi, MockStorage};
use cosmwasm_std::OwnedDeps;

use super::custom_mock_querier::CustomMockQuerier;

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, CustomMockQuerier> {
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: CustomMockQuerier::default(),
    }
}
