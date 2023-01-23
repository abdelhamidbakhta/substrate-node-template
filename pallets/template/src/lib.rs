#![cfg_attr(not(feature = "std"), no_std)]
// Custom types for the pallet.
pub mod types;
// Storage items for the pallet.
pub mod storage;
// Custom errors for the pallet.
pub mod errors;

/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::{
		pallet_prelude::{DispatchResult, *},
		traits::{Currency, Randomness},
	};
	use frame_system::pallet_prelude::{OriginFor, *};

	use crate::types::{Gender, Kitty};

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The Currency handler for the kitties pallet.
		type Currency: Currency<Self::AccountId>;

		/// The maximum number of kitties than a single account can own.
		#[pallet::constant]
		type MaxKittiesOwned: Get<u32>;

		/// The type of Randomness wee want to specify for this pallet.
		type KittyRandomness: Randomness<Self::Hash, Self::BlockNumber>;
	}

	/// Keeps track of the number of kitties in existence.
	#[pallet::storage]
	pub(super) type CountForKitties<T: Config> = StorageValue<_, u64, ValueQuery>;

	/// Maps the kitty struct to the kitty DNA.
	#[pallet::storage]
	pub(super) type Kitties<T: Config> =
		StorageMap<_, Twox64Concat, [u8; 16], crate::types::Kitty<T>>;

	/// Tracks the kitties owned by each account.
	#[pallet::storage]
	pub(super) type KittiesOwned<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<[u8; 16], T::MaxKittiesOwned>,
		ValueQuery,
	>;

	/// Custom errors for the pallet.
	#[pallet::error]
	pub enum Error<T> {
		/// An account may only own a limited number of kitties.
		TooManyOwned,
		/// This kitty already exists.
		DuplicateKitty,
		/// An overflow has occured.
		Overflow,
	}

	// Custom events for the pallet.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new kitty has been created.
		Created { kitty: [u8; 16], owner: T::AccountId },
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new kitty.
		#[pallet::weight(0)]
		pub fn create_kitty(origin: OriginFor<T>) -> DispatchResult {
			// Make sure the caller is from a signed origin and retrieve the signer.
			let sender = ensure_signed(origin)?;
			// Generate unique DNA and Gender.
			let (kitty_gen_dna, gender) = Self::gen_dna();

			// Write the new kitty to storage.
			Self::mint(&sender, kitty_gen_dna, gender)?;
			Ok(())
		}
	}

	/// The pallet internal functions.
	impl<T: Config> Pallet<T> {
		/// Generates and returns DNA and Gender.
		fn gen_dna() -> ([u8; 16], Gender) {
			// Create randomness
			let random = T::KittyRandomness::random(&b"dna"[..]).0;

			// Create randomness payload. Multiple kitties can be generated in the same blockm
			// retaining uniqueness.
			let unique_payload = (
				random,
				frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
				frame_system::Pallet::<T>::block_number(),
			);

			// Turns into a byte array.
			let encoded_payload = unique_payload.encode();
			let hash = frame_support::Hashable::blake2_128(&encoded_payload);
			match hash[0] % 2 == 0 {
				true => (hash, Gender::Male),
				false => (hash, Gender::Female),
			}
		}

		/// Mint a kitty.
		pub fn mint(
			owner: &T::AccountId,
			dna: [u8; 16],
			gender: Gender,
		) -> Result<[u8; 16], DispatchError> {
			let kitty = Kitty::<T> { dna, price: None, gender, owner: owner.clone() };

			// Check if the kitty already exists.
			let already_exists = Kitties::<T>::contains_key(&kitty.dna);
			// Dispatch error if the kitty already exists.
			ensure!(!already_exists, Error::<T>::DuplicateKitty);

			let count = CountForKitties::<T>::get();
			let new_count = count.checked_add(1).ok_or(Error::<T>::Overflow)?;

			// Insert the kitty into the bounded vec for the owner.
			KittiesOwned::<T>::try_append(&owner, kitty.dna)
				.map_err(|_| Error::<T>::TooManyOwned)?;

			// Insert the kitty into the kitties storage map.
			Kitties::<T>::insert(&kitty.dna, &kitty);
			CountForKitties::<T>::put(new_count);

			// Emit the event.
			Self::deposit_event(Event::Created { kitty: kitty.dna, owner: owner.clone() });

			// Return the DNA of the created kitty if it succeeds.
			Ok(dna)
		}
	}
}
