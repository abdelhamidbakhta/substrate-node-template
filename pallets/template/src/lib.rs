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
		traits::{Currency, ExistenceRequirement, Randomness},
	};
	use frame_system::pallet_prelude::{OriginFor, *};

	use crate::types::{BalanceOf, Gender, Kitty};

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
		/// The kitty does not exist.
		NoKitty,
		/// The kitty is not owned by the sender.
		NotOwner,
		/// Trying to transfer to the same owner.
		TransferToSelf,
		/// Ensures that the buying price is greater than the asking price.
		BidPriceTooLow,
		/// This kitty is not for sale.
		NotForSale,
		NotEnoughBalance,
	}

	// Custom events for the pallet.
	// https://docs.substrate.io/main-docs/build/events-errors/
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new kitty has been created.
		Created { kitty: [u8; 16], owner: T::AccountId },
		/// A new kitty has been transferred.
		Transferred { kitty: [u8; 16], from: T::AccountId, to: T::AccountId },
		/// When the price of a kitty is set.
		PriceSet { kitty: [u8; 16], price: Option<BalanceOf<T>> },
		/// A new kitty has been sold.
		Sold { seller: T::AccountId, buyer: T::AccountId, kitty: [u8; 16], price: BalanceOf<T> },
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

		/// Transfer a new kitty.
		#[pallet::weight(0)]
		pub fn transfer_kitty(
			origin: OriginFor<T>,
			kitty_dna: [u8; 16],
			to: T::AccountId,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin and retrieve the signer.
			let sender = ensure_signed(origin)?;

			// Perform the transfer.
			Self::transfer(&sender, &to, kitty_dna)?;

			Ok(())
		}

		/// Set the price of a kitty.
		#[pallet::weight(0)]
		pub fn set_price_kitty(
			origin: OriginFor<T>,
			kitty_dna: [u8; 16],
			price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin and retrieve the signer.
			let sender = ensure_signed(origin)?;

			Self::set_price(&sender, kitty_dna, price)?;

			Ok(())
		}

		/// Buy a kitty.
		#[pallet::weight(0)]
		pub fn buy_kitty(
			origin: OriginFor<T>,
			to: T::AccountId,
			kitty_dna: [u8; 16],
			price: BalanceOf<T>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin and retrieve the signer.
			let sender = ensure_signed(origin)?;

			Self::buy(kitty_dna, &sender, &to, price)?;

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
		fn mint(
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

		/// Transfer a kitty.
		fn transfer(
			from: &T::AccountId,
			to: &T::AccountId,
			kitty_id: [u8; 16],
		) -> Result<(), DispatchError> {
			// Check if the kitty exists.
			let kitty = Kitties::<T>::get(&kitty_id).ok_or(Error::<T>::NoKitty)?;
			// Check if the sender is the owner of the kitty.
			ensure!(kitty.owner == *from, Error::<T>::NotOwner);

			// Ensure the sender is not transferring to themselves.
			ensure!(kitty.owner != *to, Error::<T>::TransferToSelf);

			// Remove the kitty from the bounded vec for the sender.
			KittiesOwned::<T>::try_mutate_exists(from, |kitties| -> Result<(), DispatchError> {
				// Get the kitties owned by the sender.
				let kitties = kitties.as_mut().ok_or(Error::<T>::NoKitty)?;
				// Get the position of the kitty in the bounded vec.
				let pos =
					kitties.iter().position(|k| *k == kitty_id).ok_or(Error::<T>::NotOwner)?;
				// Remove the kitty from the bounded vec.
				kitties.remove(pos);
				Ok(())
			})?;

			// Insert the kitty into the bounded vec for the receiver.
			KittiesOwned::<T>::try_append(&to, kitty_id).map_err(|_| Error::<T>::TooManyOwned)?;

			// Update the owner of the kitty.
			Kitties::<T>::mutate(&kitty_id, |kitty| {
				if let Some(kitty) = kitty {
					kitty.owner = to.clone();
					// Set price to None.
					kitty.price = None;
				}
			});

			// Emit the event.
			Self::deposit_event(Event::Transferred {
				kitty: kitty_id,
				from: from.clone(),
				to: to.clone(),
			});

			Ok(())
		}

		/// Set the price of a kitty.
		fn set_price(
			from: &T::AccountId,
			kitty_id: [u8; 16],
			price: Option<BalanceOf<T>>,
		) -> Result<(), DispatchError> {
			// Check if the kitty exists.
			let kitty = Kitties::<T>::get(&kitty_id).ok_or(Error::<T>::NoKitty)?;
			// Check if the sender is the owner of the kitty.
			ensure!(kitty.owner == *from, Error::<T>::NotOwner);

			// Update the price of the kitty.
			Kitties::<T>::mutate(&kitty_id, |kitty| {
				if let Some(kitty) = kitty {
					kitty.price = price;
				}
			});

			// Emit the event.
			Self::deposit_event(Event::PriceSet { kitty: kitty_id, price });
			Ok(())
		}

		/// Buy a kitty.
		fn buy(
			kitty_dna: [u8; 16],
			seller: &T::AccountId,
			buyer: &T::AccountId,
			buy_price: BalanceOf<T>,
		) -> Result<(), DispatchError> {
			// Check if the buyer has enough balance.
			ensure!(T::Currency::free_balance(buyer) >= buy_price, Error::<T>::NotEnoughBalance);

			// Read the kitty in the storage.
			let kitty = Kitties::<T>::get(&kitty_dna).ok_or(Error::<T>::NoKitty)?;
			// Check that the kitty is for sale.
			ensure!(kitty.price.is_some(), Error::<T>::NotForSale);
			// Check that buy price is equal to or greater than the kitty's price.
			ensure!(kitty.price.unwrap() <= buy_price, Error::<T>::BidPriceTooLow);

			// Transfer the kitty to the buyer.
			Self::transfer(seller, buyer, kitty_dna)?;

			// Transfer the money to the seller.
			T::Currency::transfer(buyer, seller, buy_price, ExistenceRequirement::KeepAlive)?;

			// Emit the event.
			Self::deposit_event(Event::Sold {
				seller: seller.clone(),
				buyer: buyer.clone(),
				kitty: kitty_dna,
				price: buy_price,
			});
			Ok(())
		}
	}
}
