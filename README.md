> _**NOTE: This README is still under construction. Contents may be outdated**_

# `cw-asset`

Helpers for interacting with Cosmos assets, including native coins and CW20 tokens

## Usage

This crate contains three struct types:

- `AssetInfo` stores key information of an asset type – for CW20 tokens, the contract address; for native coins, the denomination

- `Asset` represents an asset of specific amount

- `AssetList` is a wrapper for `Vec<Asset>` which allows carrying out operations on multiple assets

Instances of `AssetInfo` and `Asset` can be created as follows:

```rust
use cw_asset::{AssetInfo, Asset};

// native coin
let coin_info = AssetInfo::native("uusd");

let coin = Asset::new(coin_info, 69420);
// or
let coin = Asset::native("uusd", 69420);

// CW20 token
let token_info = AssetInfo::cw20(deps.api.addr_validate("mock_token")?);

let token = Asset::new(token_info, 12345);
// or
let token = Asset::cw20(deps.api.addr_validate("mock_token")?, 12345);
```

### Checked and unchecked types

`AssetInfo` and `Asset` contain contract addresses of `cosmwasm_std::Addr` type. Additionally, they each comes with an "unchecked" counterpart where the addresses are in `String` type. Both the unchecked and chekced types can be serialized to / deserialized from JSON format. **The checked type is intended to be saved in contract storage, while the unchecked type is intended to be passed between contracts in messages.**

The following code snippets show common usage of the `AssetInfo` type. However, the same methods are also implemented for `Asset` type.

#### Save the checked type in storage

```rust
use cw_storage_plus::Item;

const TOKEN_INFO: Item<AssetInfo> = Item::new("token_info");

let token_info = AssetInfo::cw20(token_addr);
TOKEN_INFO.save(deps.storage, &token_info)?;
```

#### Using the unchecked type in messages

```rust
use cw_asset::AssetInfoUnchecked;

pub struct InstantiateMsg {
    token_info: AssetInfoUnchecked,
}
```

#### Conversions between checked and unchecked types

```rust
// cast checked to unchecked type
let token_info_unchecked: AssetInfoUnchecked = token_info.into();

// cast unchecked to checked type
let token_info = token_info_unchecked.check(deps.api)?;
```

### Tax handling

Stability fee (a.k.a. "tax") is a fees charged on Terra stablecoin transfers and considered by many developers to be tricky to work with.

Tax works as follows. Suppose Alice sends Bob 100 UST when the tax rate is 0.1%. The tax amount is 100 \* 0.1% = 0.1 UST. After the transfer is executed, Bob's balance increases by 100 UST, while Alice's balance is deducted by 100.1 UST.

Note that tax is paid by whoever sends the `BankMsg::Send` message, not the transaction's initiator. If Alice holds some funds in a smart contract, and invokes a functions on the contract to send 100 UST. The resulting 0.1 UST tax is deducted from the contract's balance, not Alice's.

An implication of this is that if the contract _only_ has 100 UST balance, it is impossible for it to send all 100 UST out, because it needs to reserve some funds to pay tax. In fact, at 0.1% tax rate, the maximum amount the contract can send is `99900099uusd`, with `99900uusd` needed for tax. After this transfer, the contract will have exactly `1uusd` left, which cannot be transferred out.

The `Asset` type implements two helper functions for handling taxes:

#### `deduct_tax`

Calculates the deliverable amount (tax deducted) when sending an asset:

```rust
let coin = Asset::native("uusd", 100000000);
let coin_after_tax = coin.deduct_tax(&deps.querier)?;
// at 0.1% tax rate, `coin_after_tax.amount` should be 99900099
```

#### `add_tax`

Calculates the total cost (including tax) for sending an asset:

```rust
let coin = Asset::native("uusd", 99900099);
let coin_with_tax = coin.add_tax(&deps.querier)?;
// at 0.1% tax rate, `coin_with_tax.amount` should be 99999999
```

### Message generation

The `Asset` type also comes with helper functions for generating messages:

#### `transfer_msg`

The following example creates a message for transferring 100 UST to Bob. Note that we first deduct tax before generating the message:

```rust
let coin = Asset::native("uusd", 100000000);
let msg = coin.deduct_tax(&deps.querier)?.transfer_msg("bob_address")?;
let res = Response::new().add_message(msg);
```

#### `transfer_from_msg`

The following example creates a message that draws 100 MIR tokens from Alice's wallet to Bob's. Note that:

- Alice must have approved Bob to spend her tokens using CW20's `IncreaseAllowance` command

- Invoking `transfer_from_msg` on an native coin will result in error, as native coins don't have the `TransferFrom` method

```rust
let token = Asset::cw20(deps.api.addr_validate("mock_token")?, 100000000);
let msg = token.transfer_from_msg("alice", "bob")?;
let res = Response::new().add_message(msg);
```

### Stringification

The [`std::fmt::Display`](https://doc.rust-lang.org/std/fmt/trait.Display.html) trait is implemented for `AssetInfo` and `Asset`, so you can easily invoke `to_string` method to generate a string representation of the asset. This may be useful when creating logging outputs:

```rust
let res = Response::new()
    .add_message(token.transfer_msg("alice")?)
    .add_attribute("asset_sent", token.to_string());
```

The string representation of the asset is `label:amount` where `label` is the denom for native coins, or the contract address for CW20 tokens.

### Asset list

`AssetList` is a wrapper of `Vec<Asset>` which allows you to carry out operations on multiple assets at once. For example, to send both a native coin and a CW20 token to Alice:

```rust
use cw_asset::{Asset, AssetList};

let mut assets = AssetList::new();
assets.add(Asset::native("uusd", 12345));
assets.add(Asset::cw20(api.addr_validate("mock_token")?, 67890));

let msgs = assets.deduct_tax(&deps.querier)?.transfer_msgs("alice")?;
let res = Response::new().add_messages(msgs);
```

## License

Contents of this repository are open source under [MIT License](./LICENSE).
