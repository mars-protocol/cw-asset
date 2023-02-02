use std::marker::PhantomData;

use cosmwasm_std::{
    testing::{MockApi, MockStorage},
    OwnedDeps,
};

use super::custom_mock_querier::CustomMockQuerier;

pub fn mock_dependencies() -> OwnedDeps<MockStorage, MockApi, CustomMockQuerier> {
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: CustomMockQuerier::default(),
        custom_query_type: PhantomData,
    }
}
