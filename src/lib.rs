//! # Bets Pallet
//!
//! A simple Substrate pallet that allows each account to play both the role of better and bookmaker.
//!
//! ## Overview
//!
//! The module allows each user to create a match to bet on and to place bets in matches created by other users,
//! through the following dispatchable functions: 
//!
//! * **create_match:** Passing as arguments the ID of the external match, and the odds,
//! 	it creates a match on which to act as a bookmaker and let other users bet on this.
//! * **place_bet:** Allows a user to bet on an open match. To do this, the user need to select the ID of the match
//! 	on which bet on, the predicted result and the amount wagered. Once the transaction and the bet have been submitted,
//! 	an amount equal to the bet one will be reserved in the bettor's account, an amount equal to the bet one multiplied
//! 	by the established odds will be reserved in the bookmaker's account.
//! * **set_match_result:** Retrieves the match result and saves it in storage. Subsequently, based on the latter,
//! 	it scrolls all the bets related to that match and establishes the outcome, unreserving the entire amount of the bet
//! 	to the winner (bettor or bookmaker). N.B.:
//!     	* This call that can be made by any user at the moment, should be scheduled after the end of the event,
//! 		saving the end-of-event timestamp among the match data.
//!     	* The retrieval of a match result should be done through HTTP request using an ocw. To simplify this function,
//! 		the RandomnessCollectiveFlip implementation of Randomness was used to generate the scores of the teams.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use codec::{Decode, Encode};
use frame_support::{
	dispatch::{DispatchResult},
	ensure,
	pallet_prelude::*,
	traits::{Currency, Get, ReservableCurrency, BalanceStatus, Randomness},
	PalletId, RuntimeDebug,
};
pub use pallet::*;
use frame_support::sp_runtime::{
	traits::{Saturating},
	Percent,
};
use sp_std::prelude::*;
//pub use weights::WeightInfo;

/// An index of a Match
pub type MatchIndex = u64;
/// An index of a Bet
pub type BetIndex = u64;
type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
type BalanceOf<T> = <<T as Config>::Currency as Currency<AccountIdOf<T>>>::Balance;
type BetInfoOf<T> = Bet<AccountIdOf<T>, BalanceOf<T>>;
/// Odd touple composed by integer e fractional part through Percent
type Odd = (u32, u8);

#[derive(
	Encode, Decode, Default, Clone, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq, Copy,
)]

/// A Match have an initial state (Open), and 2 final states (Closed, Postponed).
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
	/// The owner of a Match, account who accept bets with conditions below.
	pub owner: AccountId,
	/// The id of external event. Will be used by ocw to retrieve match result.
	pub id_event: u32,
	/// The status of the match : open, closed or postponed.
	pub status: MatchStatus,
	pub home_score: u32,
	pub away_score: u32,
	pub odd_homewin: Odd,
	pub odd_awaywin: Odd,
	pub odd_draw: Odd,
	pub odd_under: Odd,
	pub odd_over: Odd,
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
	Open,
	Lost,
	Won,
}

#[derive(
	Encode, Decode, Default, RuntimeDebug, MaxEncodedLen, TypeInfo, PartialEq,
)]
pub struct Bet<AccountId, Balance> {
	/// The owner of the bet, bettor account.
	pub owner: AccountId,
	/// Reference to the match on which bet on.
	pub id_match: MatchIndex,
	/// Result prediction.
	pub prediction: Prediction,
	/// Save Odd value at the moment of Bet (Odds could changhe).
	pub odd: Odd,
	/// The amount wagered.
	pub amount: Balance,
	/// The status of the bet
	pub status: BetStatus,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The bets pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;
		/// Event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// The currency mechanism.
		type Currency: ReservableCurrency<Self::AccountId>;
		/// Something that provides randomness in the runtime.
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	#[pallet::storage]
	/// Mapping matches using match_index as key.
	#[pallet::getter(fn matches_by_id)]
	pub(super) type Matches <T> =
		StorageMap<_, Blake2_128Concat, MatchIndex, SingleMatch<AccountIdOf<T>>, OptionQuery>;

	/// Mapping bets using bet_index as key.
	#[pallet::storage]
	#[pallet::getter(fn bets_by_id)]
	pub(super) type Bets<T: Config> =
		StorageMap<_, Blake2_128Concat, BetIndex, BetInfoOf<T>, OptionQuery>;

	/// A Storage Double Map of bets. Referenced by the id match to which it refers
	/// (to quickly find all the bets related to a specific match), and its index.
	// #[pallet::storage]
	// #[pallet::getter(fn bets)]
	// pub(super) type Bets<T: Config> =
	// 	StorageDoubleMap<_, Blake2_128Concat, u32, Blake2_128Concat, BetIndex, BetInfoOf<T>, OptionQuery>;

	/// Auto-incrementing match counter
	#[pallet::storage]
	#[pallet::getter(fn matches_count)]
	pub(super) type MatchCount<T: Config> = StorageValue<_, MatchIndex, ValueQuery>;
	/// Auto-incrementing bet counter
	#[pallet::storage]
	#[pallet::getter(fn bets_count)]
	pub(super) type BetCount<T: Config> = StorageValue<_, BetIndex, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A match was created.
		MatchCreated(MatchIndex),
		/// A bet was placed.
		BetPlaced(BetIndex),
		/// A match was closed.
		MatchClosed(MatchIndex),
		/// A match was closed.
		BetClaimed(BetIndex),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// A specific match does not exist, cannot place a bet or set match result.
		MatchNotExists,
		/// Bet owner and match owner must be different.
		SameMatchOwner,
		/// Fractional part of the Odd out of bound, must be into <0...99> range.
		OddFracPartOutOfBound,
		/// Integer part of the Odd out of bound, must be 1 <= odd.0 <= 4_294_967_295u32.
		OddIntPartOutOfBound,
		/// Match not open for bets or updates, functions available only on open matches.
		MatchClosed,
		/// Match open during a bet claim.
		MatchOpen,
		/// Insufficient free-balance to offer a bet.
		MatchAccountInsufficientBalance,
		/// Insufficient free-balance to place a bet.
		BetAccountInsufficientBalance,
		/// A specific bet does not exists.
		BetNotExists,
		/// Match not open for bets or updates, functions available only on open matches.
		BetClosed,
		/// Payoff procedure failed.
		PayoffError,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Passing as arguments the ID of the external match, and the odds,
		/// it creates a match on which to act as a bookmaker and let other users bet on this.
		#[pallet::weight(10_000)]
		pub fn create_match(
			origin: OriginFor<T>,
			id_event: u32,
			odd_homewin: Odd,
			odd_awaywin: Odd,
			odd_draw: Odd,
			odd_under: Odd,
			odd_over: Odd,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			// This function will return an error if the extrinsic is not signed.
			let owner = ensure_signed(origin)?;
			// Check fractional part of Odd into <0...99> range.
			ensure!(odd_homewin.1 < 99 && odd_awaywin.1 < 99 && odd_draw.1 < 99 && odd_under.1 < 99 && odd_over.1 < 99, Error::<T>::OddFracPartOutOfBound);
			// Check integer part of Odd >= 1.
			ensure!(odd_homewin.0 > 0 && odd_awaywin.0 > 0 && odd_draw.0 > 0 && odd_under.0 > 0 && odd_over.0 > 0, Error::<T>::OddIntPartOutOfBound);

			let single_match = SingleMatch {
				owner,
				id_event,
				status: MatchStatus::Open,
				home_score: 0, 
				away_score: 0,
				odd_homewin,
				odd_awaywin,
				odd_draw,
				odd_under,
				odd_over,
			};

			let match_index = Self::matches_count();
			// Store the match with id_match as key.
			<Matches<T>>::insert(match_index, single_match);
			// Not protected against overflow.
			MatchCount::<T>::put(match_index + 1);

			Self::deposit_event(Event::MatchCreated(match_index));
			Ok(())
		}

		/// Allows a user to bet on an open match. To do this, the user need to select the ID of the match
		/// on which bet on, the predicted result and the amount wagered. Once the transaction and the bet have been submitted,
		/// an amount equal to the bet one will be reserved in the bettor's account, an amount equal to the bet one multiplied
		/// by the established odds will be reserved in the bookmaker's account.
		#[pallet::weight(10_000)]
		pub fn place_bet(
			origin: OriginFor<T>,
			id_match: MatchIndex,
			prediction: Prediction,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			// Check that the extrinsic was signed and get the signer.
			let bet_owner = ensure_signed(origin)?;
			let bet_index = BetCount::<T>::get();
			// Retrieve the match struct and match_owner
			let selected_match = Self::matches_by_id(id_match).ok_or(Error::<T>::MatchNotExists)?;
			let match_owner = selected_match.owner;
			// Ensure bet owner and match owner are not the same account.
			ensure!(bet_owner != match_owner, Error::<T>::SameMatchOwner);
			// Ensure match is open.
			ensure!(selected_match.status == MatchStatus::Open, Error::<T>::MatchClosed);
			// Ensure that bettor account have suffient free balance.
			ensure!(T::Currency::can_reserve(&bet_owner, amount), Error::<T>::BetAccountInsufficientBalance);

			let odd: Odd = match prediction {
				Prediction::Homewin => selected_match.odd_homewin,
				Prediction::Awaywin => selected_match.odd_awaywin,
				Prediction::Draw => selected_match.odd_draw,
				Prediction::Over => selected_match.odd_over,
				Prediction::Under => selected_match.odd_under,
			};

			let winnable_amount = (Percent::from_percent(odd.1) * amount).saturating_add(amount.saturating_mul((odd.0 as u32).into()));
			ensure!(T::Currency::can_reserve(&match_owner, winnable_amount), Error::<T>::MatchAccountInsufficientBalance);
			T::Currency::reserve(&bet_owner, amount)?;
			T::Currency::reserve(&match_owner, winnable_amount)?;

			let bet = Bet {
				owner: bet_owner,
				id_match,
				prediction,
				odd,
				amount,
				status: BetStatus::Open,
			};
			
			// Insert bet into its storage double map.
			<Bets<T>>::insert(bet_index, bet);
			// Not protected against overflow.
			BetCount::<T>::put(bet_index + 1);

			Self::deposit_event(Event::BetPlaced(bet_index));
			Ok(().into())
		}

		/// Saves the match result into storage. At the moment the results are generated randomly,
		/// in future developments it can be called by the oracle.
		#[pallet::weight(10_000)]
		pub fn set_match_result(
			origin: OriginFor<T>,
			id_match: MatchIndex,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			let mut selected_match = Self::matches_by_id(id_match).ok_or(Error::<T>::MatchNotExists)?;
			// Check if match is open.
			ensure!(selected_match.status == MatchStatus::Open, Error::<T>::MatchClosed);
			// Update match status and results.
			// todo: randomize also MatchStatus.
			selected_match.status = MatchStatus::Closed;
			selected_match.home_score = Self::generate_random_score(0);
			selected_match.away_score = Self::generate_random_score(1);
			<Matches<T>>::insert(id_match, selected_match);
			// todo: maybe can try also this way: <Matches<T>>::try_mutate, instead of insert.
			
			Self::deposit_event(Event::MatchClosed(id_match));
			Ok(().into())
		}

		/// Settles a bet, unlocking all funds towards the winner.
		#[pallet::weight(10_000)]
		pub fn claim_bet(
			origin: OriginFor<T>,
			id_bet: BetIndex,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			let mut selected_bet = Self::bets_by_id(id_bet).ok_or(Error::<T>::BetNotExists)?;
			// Check if bet is open.
			ensure!(selected_bet.status == BetStatus::Open, Error::<T>::BetClosed);
			let selected_match = Self::matches_by_id(selected_bet.id_match).ok_or(Error::<T>::MatchNotExists)?;
			// Check if match is open.
			ensure!(selected_match.status == MatchStatus::Closed || selected_match.status == MatchStatus::Postponed, Error::<T>::MatchOpen);
			let bet_status: BetStatus = match selected_bet.prediction {
				Prediction::Homewin if selected_match.home_score > selected_match.away_score => BetStatus::Won,
				Prediction::Awaywin if selected_match.home_score < selected_match.away_score => BetStatus::Won,
				Prediction::Draw if selected_match.home_score == selected_match.away_score => BetStatus::Won,
				Prediction::Over if selected_match.home_score + selected_match.away_score > 3 => BetStatus::Won,
				Prediction::Under if selected_match.home_score + selected_match.away_score < 3 => BetStatus::Won,
				_ => BetStatus::Lost,
			};
			let winnable_amount = (Percent::from_percent(selected_bet.odd.1) * selected_bet.amount).saturating_add(selected_bet.amount.saturating_mul((selected_bet.odd.0 as u32).into()));
			// Pay off the bet.
			let match_owner = selected_match.owner.clone();
			if bet_status == BetStatus::Won {
				T::Currency::repatriate_reserved(&match_owner, &(selected_bet.owner), winnable_amount, BalanceStatus::Free)?;
				T::Currency::repatriate_reserved(&(selected_bet.owner), &match_owner, selected_bet.amount, BalanceStatus::Free)?;
			} else {
				T::Currency::repatriate_reserved(&(selected_bet.owner), &match_owner, selected_bet.amount.clone(), BalanceStatus::Free)?;
				T::Currency::unreserve(&match_owner, winnable_amount);
			}
			
			// Change bet status and save.
			selected_bet.status = bet_status;
			<Bets<T>>::insert(id_bet, selected_bet);
			
			Self::deposit_event(Event::BetClaimed(id_bet));
			Ok(().into())
		}

		// /// Retrieves the match result and saves it in storage. Subsequently, based on the latter,
		// /// it scrolls all the bets related to that match and establishes the outcome, unreserving the entire amount of the bet
		// /// to the winner (bettor or bookmaker). N.B.:
		// ///     * This call that can be made by any user at the moment, should be scheduled after the end of the event,
		// /// 	saving the end-of-event timestamp among the match data.
		// ///     * The retrieval of a match result should be done through HTTP request using an ocw. To simplify this function,
		// /// 	the RandomnessCollectiveFlip implementation of Randomness was used to generate the scores of the teams.
		// #[pallet::weight(10_000)]
		// pub fn set_match_result_old(
		// 	origin: OriginFor<T>,
		// 	id_match: u32,
		// ) -> DispatchResult {
		// 	let _who = ensure_signed(origin)?;
		// 	let mut selected_match = Self::matches_by_id(id_match).ok_or(Error::<T>::MatchNotExists)?;
		// 	// Check if match is open.
		// 	ensure!(selected_match.status == MatchStatus::Open, Error::<T>::MatchClosed);
		// 	let match_owner = selected_match.owner.clone();
		// 	let selected_bets = <Bets<T>>::iter_prefix_values(id_match);
		// 	// Update match status and results.
		// 	// todo: randomize also MatchStatus.
		// 	selected_match.status = MatchStatus::Closed;
		// 	selected_match.home_score = Self::generate_random_score(0);
		// 	selected_match.away_score = Self::generate_random_score(1);
		// 	<Matches<T>>::insert(id_match, selected_match.clone());
		// 	// todo: maybe can try also this way: <Matches<T>>::try_mutate, instead of insert.
			
		// 	// Check the winning status of the bet compared to match results.
		// 	let mut payoff_result = true;
		// 	selected_bets.for_each(|bet|{
		// 		let bet_status: BetStatus = match &bet.prediction {
		// 			Prediction::Homewin if selected_match.home_score > selected_match.away_score => BetStatus::Won,
		// 			Prediction::Awaywin if selected_match.home_score < selected_match.away_score => BetStatus::Won,
		// 			Prediction::Draw if selected_match.home_score == selected_match.away_score => BetStatus::Won,
		// 			Prediction::Over if selected_match.home_score + selected_match.away_score > 3 => BetStatus::Won,
		// 			Prediction::Under if selected_match.home_score + selected_match.away_score < 3 => BetStatus::Won,
		// 			_ => BetStatus::Lost,
		// 		};
		// 		let winnable_amount = (Percent::from_percent(bet.odd.1) * bet.amount).saturating_add(bet.amount.saturating_mul((bet.odd.0 as u32).into()));
		// 		// Pay off the bet.
		// 		if bet_status == BetStatus::Won {
		// 			let repatriate_result_mtob = T::Currency::repatriate_reserved(&match_owner, &(bet.owner), winnable_amount, BalanceStatus::Free).is_ok();
		// 			let repatriate_result_btom = T::Currency::repatriate_reserved(&(bet.owner), &match_owner, bet.amount, BalanceStatus::Free).is_ok();
		// 			if repatriate_result_mtob == false || repatriate_result_btom == false {
		// 				payoff_result = false;
		// 			}
		// 		} else {
		// 			let repatriate_result = T::Currency::repatriate_reserved(&(bet.owner), &match_owner, bet.amount.clone(), BalanceStatus::Free).is_ok();
		// 			let unreserve_result = T::Currency::unreserve(&match_owner, winnable_amount);
		// 			if repatriate_result == false || !unreserve_result.is_zero() {
		// 				payoff_result = false;
		// 			}
		// 		}
				
		// 		// change bet status and save.
		// 		//bet.status = new_bet_status;
		// 		//let key : u8 = selected_bets.last_raw_key();
		// 		//<Bets<T>>::insert(id_match, selected_bets.last_raw_key(),bet);
		// 	});
		// 	ensure!(payoff_result == true, Error::<T>::PayoffError);
		// 	Self::deposit_event(Event::MatchClosed(id_match));
		// 	Ok(().into())
		// }

	}
}

impl<T: Config> Pallet<T> {
	/// generate a random score for a match, some code from an internal function of lottery pallet.
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

	/// generate a random number, internal function from lottery pallet.
	fn generate_random_number(seed: u32) -> u32 {
		let (random_seed, _) = T::Randomness::random(&(T::PalletId::get(), seed).encode());
		let random_number = <u32>::decode(&mut random_seed.as_ref())
			.expect("secure hashes should always be bigger than u32; qed");
		random_number
	}
}
