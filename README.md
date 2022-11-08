# Bets Pallet

A simple Substrate pallet that allows each account to play both the role of better and bookmaker.

## Overview

The module allows each user to create a match to bet on and to place bets in matches created by other users, through the following dispatchable functions:

* **set_odds:** Passing as arguments the ID of the external match, and the odds, it creates a match on which to act as a bookmaker and let other users bet on this.
* **set_match_start:** Save the timestamp of the match start to block the placement of odds/bets at a later time. N.B.:
    * The retrieval of a match start time is done through HTTP request using an OCW, this one will call then the *set_match_start* extrinsic.
* **place_bet:** Allows a user to bet on an open match. To do this, the user need to select the ID of the match on which bet on, the predicted result and the amount wagered. Once the transaction and the bet have been submitted, an amount equal to the bet one will be reserved in the bettor's account, an amount equal to the 'bet one multiplied by the (established odds - 1)', called *winnable_amount*, will be reserved in the bookmaker's account.
* **set_match_result:** Retrieves the match result and saves it in storage. Subsequently, based on the latter, it scrolls all the bets related to that match and establishes the outcome, unreserving the entire amount of the bet to the winner (bettor or bookmaker). N.B.:
    * This call that can be made by any user at the moment, should be scheduled after the end of the event, saving the end-of-event timestamp among the match data.
    * The retrieval of a match result is done through HTTP request using an OCW, this one will call then the *set_match_result* extrinsic.
* **settle_bet:** Settle a bet after match closing. 

## Usage
The pallet can be used on a pre-customized node (starting from the base of the substrate-node-template), or integrated on your own node.
### Prerequisites
* [`Rust and the Rust toolchain`](https://docs.substrate.io/install/rust-toolchain/)

### Using the ready to start node
* Clone this repository containing a substrate-node-template fork, with a branch dedicated to the bet pallet : [`substrate-node-template`](https://github.com/mns23/substrate-node-template/tree/pallet-bets-dev-ocw)
* Checkout to pallet-bets-dev-ocw
* Check, compile and run the node through the following commands:
```shell
cargo check -p node-template-runtime

cargo build --release

./target/release/node-template --dev
```

### Using your own node (Update needed after Offchain Worker Integration)
Add this code to runtime/src/lib.rs, just before the construct_runtime! Macro
```rust
parameter_types! {
	pub const BetsPalletId: PalletId = PalletId(*b"py/bbets");
}

/// Configure the pallet-bets
impl pallet_bets::Config for Runtime {
	type PalletId = BetsPalletId;
	type Currency = Balances;
	type Event = Event;
	type Randomness = RandomnessCollectiveFlip;
}
```

Add this line to the construct_runtime!, as well as for pallets already present:
```rust
BetsModule: pallet_bets,
```

Add PalletId struct, inserting this line inside  'pub use frame_support::{'
```rust
PalletId,
```

Add the first line to Local Dependencies of runtime's Cargo.toml, then the second into the std feature
```rust
pallet-bets = { version = "1.0.0-dev", default-features = false, git = "https://github.com/mns23/bets.git", branch = "main" }

"pallet-bets/std",
```

## Related Modules

* [`System`](https://docs.rs/frame-system/latest/frame_system/)
* [`Support`](https://docs.rs/frame-support/latest/frame_support/)
* [`pallet-balances`](https://docs.rs/pallet-balances/latest/pallet_balances/)
* [`pallet-timestamp`](https://docs.rs/pallet-timestamp/latest/pallet_timestamp/)
* [`pallet-lottery`](https://docs.rs/pallet-lottery/latest/pallet_lottery/) for Randomness

## Next Steps
* Develop benchmarks and weights.
* ~~Use fixed point arithmetic for odds.~~
* Implement multi-bet, a bet based on multiple combined events.
* Think about using a Lockable currency rather than Reservable.
* ~~Implement an oracle through offchain workers~~: requires adding also an event forecasted endtime to the match struct. Some minutes after the forecasted endtime the oracle will perform HTTP request to the event tracker website and update the match status through the 'set_match_result' extrinsic, signign the transaction.

License: Unlicense