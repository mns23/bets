#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
use codec::{Decode, Encode};
use frame_support::{
	dispatch::{DispatchResult, Dispatchable, GetDispatchInfo},
	ensure,
	pallet_prelude::*,
	storage::bounded_vec::BoundedVec,
	traits::{Currency, ExistenceRequirement, Get, ReservableCurrency, WithdrawReasons, BalanceStatus, Randomness},
	PalletId, RuntimeDebug,
};
pub use pallet::*;
use frame_support::sp_runtime::{
	traits::{AccountIdConversion, Saturating, Zero},
	ArithmeticError, DispatchError,
};
use sp_std::prelude::*;
//pub use weights::WeightInfo;

pub type MatchIndex = u32;
pub type BetIndex = u32;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;
type BetInfoOf<T> = Bet<AccountIdOf<T>, BalanceOf<T>>;

#[derive(
	Encode, Decode, Default, Clone, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq, Copy,
)]
pub enum MatchStatus {
	#[default]
	Open,
	Closed,
	Postponed,
}

#[derive(
	Encode, Decode, Default, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq, Clone, Copy,
)]
pub struct SingleMatch<AccountId> {
	pub owner: AccountId,
	pub id_match: u32,
	pub status: MatchStatus,
	//pub description: String,
	pub home_score: u32,
	pub away_score: u32,
	pub odd_homewin: u32,
	pub odd_awaywin: u32,
	pub odd_draw: u32,
	pub odd_under: u32,
	pub odd_over: u32,
}

#[derive(
	Encode, Decode, Default, Clone, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq,
)]
pub enum Prediction {
	#[default]
	Homewin,
	Awaywin,
	Draw,
	Under,
	Over,
}

#[derive(
	Encode, Decode, Default, Clone, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq, Copy,
)]
pub enum BetStatus {
	#[default]
	Lost,
	Won,
}

#[derive(
	Encode, Decode, Default, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq,
)]
pub struct Bet<AccountId, Balance> {
	pub owner: AccountId,
	pub id_match: u32,
	pub prediction: Prediction,
	pub odd: u32,
	pub amount: Balance,
}


#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;


	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The bets pallet id
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: ReservableCurrency<Self::AccountId>;
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	// The pallet's runtime storage items.
	// https://docs.substrate.io/main-docs/build/runtime-storage/
	#[pallet::storage]
	#[pallet::getter(fn matches_by_id)]
	pub(super) type Matches <T> =
		StorageMap<_, Blake2_128Concat, u32, SingleMatch<AccountIdOf<T>>, OptionQuery>;
	
	#[pallet::storage]
	#[pallet::getter(fn bets)]
	pub(super) type Bets<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, u32, Blake2_128Concat, BetIndex, BetInfoOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn bets_count)]
	pub(super) type BetCount<T: Config> = StorageValue<_, BetIndex, ValueQuery>;
	// Learn more about declaring storage items:
	// https://docs.substrate.io/main-docs/build/runtime-storage/#declaring-storage-items
	// pub type Something<T> = StorageValue<_, u32>;

	// Pallets use events to inform users when important changes are made.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		MatchCreated(u32),
		BetPlaced(BetIndex),
		MatchClosed(u32),
		/// Event emitted when a claim has been created.
		ClaimCreated { who: T::AccountId, claim: T::Hash },
		/// Event emitted when a claim is revoked by the owner.
		ClaimRevoked { who: T::AccountId, claim: T::Hash },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		MatchNotExists,
		NoBetExists,
		ClaimFailed,
		/// The claim already exists.
		AlreadyClaimed,
		/// The claim does not exist, so it cannot be revoked.
		NoSuchClaim,
		/// The claim is owned by another account, so caller can't revoke it.
		NotClaimOwner,
	}

	// Dispatchable functions allow users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_match(
			origin: OriginFor<T>,
			id_match: u32,
			odd_homewin: u32,
			odd_awaywin: u32,
			odd_draw: u32,
			odd_under: u32,
			odd_over: u32,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			let owner = ensure_signed(origin)?;

			// Verify that the specified claim has not already been stored.
			//ensure!(!Claims::<T>::contains_key(&claim), Error::<T>::AlreadyClaimed);

			// Get the block number from the FRAME System pallet.
			//let current_block = <frame_system::Pallet<T>>::block_number();
			let single_match = SingleMatch {
				owner,
				id_match,
				status: MatchStatus::Open,
				home_score: 0, 
				away_score: 0,
				odd_homewin,
				odd_awaywin,
				odd_draw,
				odd_under,
				odd_over,
			};
			// Store the claim with the sender and block number.
			<Matches<T>>::insert(id_match, single_match);

			Self::deposit_event(Event::MatchCreated(id_match));

			Ok(())
		}

		#[pallet::weight(10_000)]
		pub fn place_bet(
			origin: OriginFor<T>,
			id_match: u32,
			prediction: Prediction,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let bet_owner = ensure_signed(origin)?;

			//ensure!(end > now, <Error<T>>::EndTooEarly);

			let index = BetCount::<T>::get();
			// not protected against overflow, see safemath section
			BetCount::<T>::put(index + 1);


			let selected_match = Self::matches_by_id(id_match).ok_or(Error::<T>::MatchNotExists)?;
			let match_owner = selected_match.owner;

			// T::Currency::transfer(
			// 	&bet_owner,
			// 	&match_owner,
			// 	amount,
			// 	ExistenceRequirement::AllowDeath,
			// )?;
			let odd: u32 = match prediction {
				Prediction::Homewin => selected_match.odd_homewin,
				Prediction::Awaywin => selected_match.odd_awaywin,
				Prediction::Draw => selected_match.odd_draw,
				Prediction::Over => selected_match.odd_over,
				Prediction::Under => selected_match.odd_under,
			};

			T::Currency::reserve(&bet_owner, amount)?;
			//todo: add mod arithmetic
			T::Currency::reserve(&match_owner, amount.saturating_mul((odd as u32).into()))?;

			let bet = Bet {
				owner: bet_owner,
				id_match,
				prediction,
				odd,
				amount,
			};

			<Bets<T>>::insert(id_match, index,bet);

			Self::deposit_event(Event::BetPlaced(index));
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn set_match_result(
			origin: OriginFor<T>,
			id_match: u32,
		) -> DispatchResult {
			let mut selected_match = Self::matches_by_id(id_match).ok_or(Error::<T>::MatchNotExists)?;
			let match_owner = selected_match.owner.clone();
			let selected_bets = <Bets<T>>::iter_prefix_values(id_match);
			//Check if value null

			
			//Update match status and results
			selected_match.status = MatchStatus::Closed;
			selected_match.home_score = Self::generate_random_score(0);
			selected_match.away_score = Self::generate_random_score(1);
			<Matches<T>>::insert(id_match, selected_match.clone());
			// <Matches<T>>::try_mutate(id_match, |matchh| {
			// 	*matchh = selected_match;
			// 	Ok(())
			// });
			
			//Check the winning status of the bet compared to match results
			selected_bets.for_each(|bet|{
				let bet_status: BetStatus = match &bet.prediction {
					Prediction::Homewin if selected_match.home_score > selected_match.away_score => BetStatus::Won,
					Prediction::Awaywin if selected_match.home_score < selected_match.away_score => BetStatus::Won,
					Prediction::Draw if selected_match.home_score == selected_match.away_score => BetStatus::Won,
					Prediction::Over if selected_match.home_score + selected_match.away_score > 3 => BetStatus::Won,
					Prediction::Under if selected_match.home_score + selected_match.away_score < 3 => BetStatus::Won,
					_ => BetStatus::Lost,
				};
				
				if bet_status == BetStatus::Won {
					//maybe unwrap_or
					T::Currency::repatriate_reserved(&match_owner, &(bet.owner), bet.amount.saturating_mul((bet.odd as u32).into()), BalanceStatus::Free).unwrap();
					T::Currency::unreserve(&(bet.owner), bet.amount);
				} else {
					T::Currency::repatriate_reserved(&(bet.owner), &match_owner.clone(), bet.amount, BalanceStatus::Free).unwrap();
					T::Currency::unreserve(&match_owner, bet.amount);
				}
				//change bet status and save
				//bet.status = new_bet_status;
				//let key : u8 = selected_bets.last_raw_key();
				//<Bets<T>>::insert(id_match, selected_bets.last_raw_key(),bet);
			});
			Self::deposit_event(Event::MatchClosed(id_match));
			Ok(().into())
		}

	}
}

impl<T: Config> Pallet<T> {
	///internal function from lottery pallet
	fn generate_random_score(seed_diff: u32) -> u32 {
		let mut random_number = Self::generate_random_number(seed_diff);
		let max_trials: u32 = 10;
		let max_score: u32 = 9;

		// Best effort attempt to remove bias from modulus operator.
		for i in 1..max_trials {
			if random_number < u32::MAX - u32::MAX % max_score {
				break
			}
			random_number = Self::generate_random_number(seed_diff + i);
		}

		random_number % max_score
	}

	///internal function from lottery pallet
	fn generate_random_number(seed: u32) -> u32 {
		let (random_seed, _) = T::Randomness::random(&(T::PalletId::get(), seed).encode());
		let random_number = <u32>::decode(&mut random_seed.as_ref())
			.expect("secure hashes should always be bigger than u32; qed");
		random_number
	}
}
