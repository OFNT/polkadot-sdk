//! Pallet-Project.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

// FRAME pallets require their own "mock runtimes" to be able to run unit tests. This module
// contains a mock runtime specific for testing this pallet's functionality.
#[cfg(test)]
mod mock;

// This module contains the unit tests for this pallet.
// Learn about pallet unit testing here: https://docs.substrate.io/test/unit-testing/
#[cfg(test)]
mod tests;

// Every callable function or "dispatchable" a pallet exposes must have weight values that correctly
// estimate a dispatchable's execution time. The benchmarking module is used to calculate weights
// for each dispatchable and generates this pallet's weight.rs file. Learn more about benchmarking here: https://docs.substrate.io/test/benchmark/
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

/// Funding options tailored to meet the needs of diverse proj-, change to SpendOrigin.
/// ects and investor interests.
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Funding {
	Donation,
	Rewards,
	Equity,
	Milestone,
}

/// The state of the payment claim.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub enum PaymentState {
	/// Pending claim.
	Pending,
	/// Payment attempted with a payment identifier.
	Attempted,
	/// Payment failed.
	Failed,
}

/// Info regarding an approved treasury spend.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct SpendStatus<Balance, Beneficiary, BlockNumber> {
	// The kind of asset to be spent.
	asset_kind: AssetKind,
	/// The asset amount of the spend.
	amount: Balance,
	/// The beneficiary of the spend.
	beneficiary: Beneficiary,
	/// The block number from which the spend can be claimed.
	valid_from: BlockNumber,
	/// The block number by which the spend has to be claimed.
	expire_at: BlockNumber,
	/// The status of the payout/claim.
	status: PaymentState,
}

/// An index of a proposal. Just a `u32`.
pub type ProposalIndex = u32;

/// A spending proposal.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct Proposal<AccountId, Balance> {
	/// The account proposing it.
	proposer: AccountId,
	/// The (total) amount that should be paid if the proposal is accepted.
	value: Balance,
	/// The account to whom the payment should be made if the proposal is accepted.
	beneficiary: AccountId,
	/// The amount held on deposit (reserved) for making this proposal.
	bond: Balance,
}

// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
	// Import various useful types required by all FRAME pallets.
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	// The `Pallet` struct serves as a placeholder to implement traits, methods and dispatchables
	// (`Call`s) in this pallet.
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// The pallet's configuration trait.
	///
	/// All our types and constants a pallet depends on must be declared here.
	/// These types are defined generically and made concrete when the pallet is declared in the
	/// `runtime/src/lib.rs` file of your chain.
	#[pallet::config]
	pub trait Config: frame_system::Config {

		/// Impl default here.
		/// Not an instantiable pallet(I is void here)
		/// The overarching runtime event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// A type representing the weights required by the dispatchables of this pallet.
		type WeightInfo: WeightInfo;

		//type Polls: Polling<TallyOf<Self, I>, Votes = Votes, Moment = BlockNumberFor<Self>>;

		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		type RejectOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type SpendPeriod: Get<BlockNumberFor<Self>;

		type Burn: Get<Permill>;

		type PalletId: Get<PalletId>;

		type BurnDestination: OnUnbalanced<NegativeImbalanceOf<Self>;

		type SpendFunds: SpendFunds<Self>;

		type MaxApprovals: Get<u32>;

		type SpendOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = BalanceOf<Self>>;

		type Beneficiary: Parameter + MaxEncodedLen;

		type Paymaster: Pay<Beneficiary = Self::Beneficiary, AssetKind = Self::Currency>;

		type PayoutPeriod: Get<BlockNumberFor<Self>;

		type BlockNumberProvider: BlockNumberProvider;
	}

	/// Number of proposals that have been made.
	#[pallet::storage]
	pub type ProposalCount<T> = StorageValue<_, ProposalIndex, ValueQuery>;

	/// Proposals that have been made. by managers... there have to be accepted intp the system
	#[pallet::storage]
	pub type Proposals<T: Config> = StorageMap<
		_,
		Twox64Concat,
		ProposalIndex,
		Twox64Concat,
		AccountId,
		Proposal<T::AccountId, BalanceOf<T>>,
		OptionQuery,
	>;

	/// The amount which has been reported as inactive to Currency.
	#[pallet::storage]
	pub type Deactivated<T: Config> =
		StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Proposal indices that have been approved but not yet awarded.
	#[pallet::storage]
	pub type Approvals<T> =
		StorageValue<_, BoundedVec<ProposalIndex, T::MaxApprovals>, ValueQuery>;

	/// The count of spends that have been made.
	#[pallet::storage]
	pub(crate) type SpendCount<T> = StorageValue<_, SpendIndex, ValueQuery>;

	/// Spends that have been approved and being processed.
	// Hasher: Twox safe since `SpendIndex` is an internal count based index.
	#[pallet::storage]
	pub type Spends<T: Config> = StorageMap<
		_,
		Twox64Concat,
		SpendIndex,
		SpendStatus<
			BalanceOf<T>,
			T::Beneficiary,
			BlockNumberFor<T>,
		>,
		OptionQuery,
	>;

	/// The blocknumber for the last triggered spend period.
	#[pallet::storage]
	pub(crate) type LastSpendPeriod<T> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;

	/// Add managers, only managers can decide to post a proposal, 
	/// Adding managers has to be a root or governance descision.
	/// Restric managers to their funding type.
	/// funding type can only be changed by root or governance vote in the democracy pallet.
	/// Root is sudo for testing.
	/// Project Id is proposal Id.
	/// Max proposals by funding type.. funding type is another key.
	/// Max proposal per total emissions from treasury for that emmission period.
	/// If the balance of the allowed treasuty emmision execed the max potential payout from
	/// proposals then no proposals get initiated for that emision period.
	/// vettoed proposal the lockup balance geos back into the treasury.
	/// amount to be funded back into the treasury once your milestones are met.
	/// Once your funding type is set you can't change it until all proposals close and governance votes for the change.
	#[pallet::storage]
	pub type Managers<T> = StorageDoubleMap<_, Twox64Concat, T::AccountId, Twox64Concat, Funding, Vec<Proposal<T::AccountId, BalanceOf<T>>>, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T> {
		#[serde(skip)]
		_config: core::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			// Create Treasury account
			let account_id = Pallet::<T>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
		}
	}

	pub enum Event<T: Config> {

		Spending,

		Awarded,

		Burnt,

		Rollover

		Deposit,

		SpendApproved,

		UpdatedInactive { reactivated: BalanceOf<T>, deactivated: BalanceOf<T>},

		Paid,

		PaymentFailed,

		SpendProcessed,
	}


	pub enum Error<T> {
		InvalidIndex,

		TooManyApprovals,

		InsufficientFunds,

		ProposalNotApproved,

		FailedToConvertBalance,

		SpendExpired,

		EarlyPayout,

		AlreadyAttempted,

		PayoutError,

		NotAttempted,

		Inconclusive,
	}

	#[pallet::hooks]
	impl<T> Hooks<BlockNumberFor> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let pot = Self::pot();
			let deactivated = Deactivated::<T>::get();
			if pot != deactivated { // what are this conditionla case would apply?
				T::Currency::reactivate(deactivated); // what are the cases where one would deactivate funds?
				T:;Currency::deactivate(pot); // what are the cases where pot would be deactivated?
				Deactivated::<T>::put(pot); //Why would pot be placed in deactivated Storage?
				Self::deposit_event(Event::<T>::UpdatedInactive {
					reactivated: deactivated,
					deactivated: pot,
				});
			}

			let last_spend_period = LastSpendPeriod::<T>::get().unwrap_or_else(|| Self::update_last_spend_period());
			let blocks_since_last_spend_period = block_number.saturating_sub(last_spend_period);
			let safe_spend_period = T::SpendPeriod::get().max(BlockNumberFor::<T>::one());

			// Safe because of `max(1)` above.
			let (spend_periods_passed, extra_blocks) = (
				blocks_since_last_spend_period / safe_spend_period,
				blocks_since_last_spend_period % safe_spend_period,
			);
			let new_last_spend_period = block_number.saturating_sub(extra_blocks);
			if spend_periods_passed > BlockNumberFor::<T>::zero() {
				Self::spend_funds(spend_periods_passed, new_last_spend_period)
			} else {
				Weight::zero()
			}
		}
			
		#[cfg(feature = "try-runtime")]
		fn try_state(_: BlockNumberFor<T>) -> Result<(), sp_runtime::TryRuntimeError> {
			Self::do_try_state()?;
			Ok(())
		}
	}

	// what is spendcontex in the context of the runtime?
	#[derive(Default)]
	struct SpendContext<Balance> {
		spend_in_context: BTreeMap<Balance, Balance>,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {

		// The funding manager should drop a compeling proposal for funding.
		// and have an amount 75% of the total requested funds that are locked and be vested into the treasury 
		/// when delivering the propsal at the required date fails.
		// at every milestone delivery date a percentace of the locked up funds are given to treasury swquentially until zero
		// and proposal is vettoed.
		// if a milestone is deliverd before the end date of amounts are gradually retured.

		pub fn add_manager(
			origin: OriginFor<T>,
			manager: AccountId,
			type: Funding,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;

		}

		pub fn remove_manager(
			origin: OriginFor<T>, 
			manager: AccountId,
			type: Funding,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;


		}
	}
}

impl<T: Config> Pallet<T> {

	fn update_last_spend_period() -> BlockNumberFor<T, I> {
		let block_number = T::BlockNumberProvider::current_block_number();
		let spend_period = T::SpendPeriod::get().max(BlockNumberFor::<T, I>::one());
		let time_since_last_spend = block_number % spend_period;
		// If it happens that this logic runs directly on a spend period block, we need to backdate
		// to the last spend period so a spend still occurs this block.
		let last_spend_period = if time_since_last_spend.is_zero() {
			block_number.saturating_sub(spend_period)
		} else {
			// Otherwise, this is the last time we had a spend period.
			block_number.saturating_sub(time_since_last_spend)
		};
		LastSpendPeriod::<T, I>::put(last_spend_period);
		last_spend_period
	}
}
