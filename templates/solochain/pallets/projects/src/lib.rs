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
use core::marker::PhantomData;
pub use weights::*;

extern crate alloc;

use alloc::collections::btree_map::BTreeMap;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	traits::{
		tokens::Pay, Currency, ExistenceRequirement::KeepAlive, Get, Imbalance, OnUnbalanced,
		ReservableCurrency, WithdrawReasons,
	},
	weights::Weight,
	PalletId,
};
use scale_info::TypeInfo;
use sp_runtime::{
	print,
	traits::{AccountIdConversion, BlockNumberProvider, One, UniqueSaturatedInto, Zero},
	PerThing, Permill, RuntimeDebug, Saturating,
};

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
pub type PositiveImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::PositiveImbalance;
pub type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::NegativeImbalance;
pub type BlockNumberFor<T> =
	<<T as Config>::BlockNumberProvider as BlockNumberProvider>::BlockNumber;

/// Funding options tailored to meet the needs of diverse projects
/// and investor interests. This will be adapted to spend origin.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
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

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait SpendFunds<T: Config> {
	fn spend_funds(
		budget_remaining: &mut BalanceOf<T>,
		imbalance: &mut PositiveImbalanceOf<T>,
		total_weight: &mut Weight,
		missed_any: &mut bool,
	);
}

/// Info regarding an approved treasury spend.
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
#[derive(Encode, Decode, Clone, PartialEq, Eq, MaxEncodedLen, RuntimeDebug, TypeInfo)]
pub struct SpendStatus<Balance, Beneficiary, BlockNumber> {
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
	index: ProposalIndex,
	/// The account proposing it.
	proposer: AccountId,
	/// The (total) amount that should be paid if the proposal is accepted.
	value: Balance,
	/// The account to whom the payment should be made if the proposal is accepted.
	beneficiary: AccountId,
	/// The amount held on deposit (reserved) for making this proposal.
	bond: Balance,

	funding: Funding,
}

/// Index of an approved treasury spend.
pub type SpendIndex = u32;

// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
	// Import various useful types required by all FRAME pallets.
	use super::*;
	use frame_support::{dispatch_context::with_context, pallet_prelude::*};
	use frame_system::pallet_prelude::{BlockNumberFor as SystemBlockNumberFor, OriginFor};

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

		type Currency: Currency<Self::AccountId> + ReservableCurrency<Self::AccountId>;

		type MaxProposalsPerManager: Get<u32>;

		type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type PalletId: Get<PalletId>;

		type ProposalBond: Get<Permill>;

		type SpendPeriod: Get<BlockNumberFor<Self>>;

		type Burn: Get<Permill>;

		type BurnDestination: OnUnbalanced<NegativeImbalanceOf<Self>>;

		type SpendFunds: SpendFunds<Self>;

		#[pallet::constant]
		type MaxApprovals: Get<u32>;

		type SpendOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = BalanceOf<Self>>;

		type Beneficiary: Parameter + MaxEncodedLen;

		type BlockNumberProvider: BlockNumberProvider;
	}

	/// Number of proposals that have been made.
	#[pallet::storage]
	pub type ProposalCount<T> = StorageValue<_, ProposalIndex, ValueQuery>;

	/// The amount which has been reported as inactive to Currency.
	#[pallet::storage]
	pub type Deactivated<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Proposal indices that have been approved but not yet awarded.
	#[pallet::storage]
	pub type Approvals<T: Config> =
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
		SpendStatus<BalanceOf<T>, T::Beneficiary, BlockNumberFor<T>>,
		OptionQuery,
	>;

	/// The blocknumber for the last triggered spend period.
	#[pallet::storage]
	pub(crate) type LastSpendPeriod<T: Config> = StorageValue<_, BlockNumberFor<T>, OptionQuery>;
	/// Managers allowed to submit proposals.
	#[pallet::storage]
	pub type Managers<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Twox64Concat,
		Funding,
		BoundedVec<Proposal<T::AccountId, BalanceOf<T>>, T::MaxProposalsPerManager>,
		ValueQuery,
	>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T> {
		#[serde(skip)]
		_config: core::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			// Create pallet account
			let account_id = Pallet::<T>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Spending {
			budget_remaining: BalanceOf<T>,
		},

		Burnt {
			burnt_funds: BalanceOf<T>,
		},

		Rollover {
			rollover_balance: BalanceOf<T>,
		},

		SpendApproved {
			proposal_index: ProposalIndex,
			amount: BalanceOf<T>,
			beneficiary: T::AccountId,
		},

		UpdatedInactive {
			reactivated: BalanceOf<T>,
			deactivated: BalanceOf<T>,
		},

		Awarded {
			proposal_index: ProposalIndex,
			award: BalanceOf<T>,
			account: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Manager already exists for the given funding type.
		AlreadyAManager,
		/// Cannot remove a manager with active proposals.
		ActiveProposalsExist,

		TooManyApprovals,

		ManagerNotFound,

		InsufficientPermission,

		UnauthorizedManager,

		TooManyProposals,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<SystemBlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: SystemBlockNumberFor<T>) -> Weight {
			let block_number = T::BlockNumberProvider::current_block_number();
			let pot = Self::pot();
			let deactivated = Deactivated::<T>::get();
			if pot != deactivated {
				T::Currency::reactivate(deactivated); // what are the cases where one would deactivate funds?
				T::Currency::deactivate(pot); // what are the cases where pot would be deactivated?
				Deactivated::<T>::put(pot); //Why would pot be placed in deactivated Storage?
				Self::deposit_event(Event::<T>::UpdatedInactive {
					reactivated: deactivated,
					deactivated: pot,
				});
			}

			let last_spend_period =
				LastSpendPeriod::<T>::get().unwrap_or_else(|| Self::update_last_spend_period());
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
		// and have an amount 75% of the total requested funds that are locked and be vested into
		// the treasury
		/// when delivering the propsal at the required date fails.
		// at every milestone delivery date a percentace of the locked up funds are given to
		// treasury swquentially until zero and proposal is vettoed.
		// if a milestone is deliverd before the end date of amounts are gradually retured.

		/// If a manager is already added and you want to change the funding type of said manager.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::add_manager())]
		pub fn add_manager(
			origin: OriginFor<T>,
			manager: T::AccountId,
			funding_type: Funding,
			change: bool,
		) -> DispatchResult {
			// Ensure the caller is authorized
			T::AdminOrigin::ensure_origin(origin)?;

			if change {
				// Get all funding types and proposals for this manager
				let existing_entries: Vec<(
					Funding,
					BoundedVec<Proposal<T::AccountId, BalanceOf<T>>, T::MaxProposalsPerManager>,
				)> = Managers::<T>::iter_prefix(&manager).collect();

				// Remove all existing entries for this manager
				for (old_funding, _) in existing_entries {
					Managers::<T>::remove(&manager, old_funding);
				}
			} else {
				// Check if manager already exists for this funding type
				ensure!(
					!Managers::<T>::contains_key(&manager, &funding_type),
					Error::<T>::AlreadyAManager
				);
			}

			// Insert/update with new funding type and empty proposals list
			let empty_proposals = BoundedVec::default();
			Managers::<T>::insert(&manager, funding_type, empty_proposals);

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::remove_manager())]
		pub fn remove_manager(
			origin: OriginFor<T>,
			manager: T::AccountId,
			funding_type: Funding,
		) -> DispatchResult {
			// Ensure the caller is authorized
			T::AdminOrigin::ensure_origin(origin)?;

			// Check if manager exists for this funding type
			ensure!(
				Managers::<T>::contains_key(&manager, &funding_type),
				Error::<T>::ManagerNotFound
			);

			// Check for active proposals using proper storage access
			let proposals = Managers::<T>::get(&manager, &funding_type);
			ensure!(proposals.is_empty(), Error::<T>::ActiveProposalsExist);

			// Remove manager entry with proper key references
			Managers::<T>::remove(&manager, &funding_type);

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::spend_local())]
		pub fn spend_local_milestone(
			origin: OriginFor<T>,
			#[pallet::compact] amount: BalanceOf<T>,
			beneficiary: T::AccountId,
		) -> DispatchResult {
			let max_amount = T::SpendOrigin::ensure_origin(origin)?;
			ensure!(amount <= max_amount, Error::<T>::InsufficientPermission);

			// Verify manager exists and has appropriate permissions
			ensure!(
				Managers::<T>::contains_key(&beneficiary, Funding::Milestone),
				Error::<T>::UnauthorizedManager
			);

			with_context::<SpendContext<BalanceOf<T>>, _>(|v| {
				let context = v.or_default();
				let spend = context.spend_in_context.entry(max_amount).or_default();

				if spend.checked_add(&amount).map(|s| s > max_amount).unwrap_or(true) {
					Err(Error::<T>::InsufficientPermission)
				} else {
					*spend = spend.saturating_add(amount);
					Ok(())
				}
			})
			.unwrap_or(Ok(()))?;

			// Generate new proposal index FIRST
			let proposal_index = ProposalCount::<T>::get();

			// Create new proposal with proper index and bond calculation
			let proposal = Proposal {
				index: proposal_index, // Set the index from ProposalCount
				proposer: beneficiary.clone(),
				value: amount,
				beneficiary: beneficiary.clone(),
				bond: T::ProposalBond::get().mul_floor(amount),
				funding: Funding::Milestone,
			};

			// Update approvals and proposals atomically
			Approvals::<T>::try_append(proposal_index).map_err(|_| Error::<T>::TooManyApprovals)?;

			// Get and update proposals
			Managers::<T>::try_mutate(&beneficiary, Funding::Milestone, |proposals| {
				proposals.try_push(proposal).map_err(|_| Error::<T>::TooManyProposals)
			})?;

			// Update proposal count AFTER successful insertion
			ProposalCount::<T>::put(proposal_index + 1);

			Self::deposit_event(Event::SpendApproved { proposal_index, amount, beneficiary });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn update_last_spend_period() -> BlockNumberFor<T> {
		let block_number = T::BlockNumberProvider::current_block_number();
		let spend_period = T::SpendPeriod::get().max(BlockNumberFor::<T>::one());
		let time_since_last_spend = block_number % spend_period;
		// If it happens that this logic runs directly on a spend period block, we need to backdate
		// to the last spend period so a spend still occurs this block.
		let last_spend_period = if time_since_last_spend.is_zero() {
			block_number.saturating_sub(spend_period)
		} else {
			// Otherwise, this is the last time we had a spend period.
			block_number.saturating_sub(time_since_last_spend)
		};
		LastSpendPeriod::<T>::put(last_spend_period);
		last_spend_period
	}

	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Return the amount of money in the pot.
	// The existential deposit is not part of the pot so treasury account never gets deleted.
	pub fn pot() -> BalanceOf<T> {
		T::Currency::free_balance(&Self::account_id())
			// Must never be less than 0 but better be safe.
			.saturating_sub(T::Currency::minimum_balance())
	}

	pub fn spend_funds(
		spend_periods_passed: BlockNumberFor<T>,
		new_last_spend_period: BlockNumberFor<T>,
	) -> Weight {
		LastSpendPeriod::<T>::put(new_last_spend_period);
		let mut total_weight = Weight::zero();
		let mut budget_remaining = Self::pot();
		let account_id = Self::account_id();
		let mut imbalance = PositiveImbalanceOf::<T>::zero();
		let mut missed_any = false;

		Self::deposit_event(Event::Spending { budget_remaining });

		// Process all manager proposals
		let processed_proposals = Managers::<T>::iter()
			.filter_map(|(manager, funding, mut proposals)| {
				let approved_indices = Approvals::<T>::get();
				let mut to_remove = Vec::new();

				// Iterate in reverse to avoid index shifting
				for (idx, proposal) in proposals.iter().enumerate().rev() {
					if approved_indices.binary_search(&proposal.index).is_ok() {
						if proposal.value <= budget_remaining {
							budget_remaining -= proposal.value;

							// Return deposit
							let err_amount =
								T::Currency::unreserve(&proposal.proposer, proposal.bond);
							debug_assert!(err_amount.is_zero());

							// Create imbalance
							imbalance.subsume(T::Currency::deposit_creating(
								&proposal.beneficiary,
								proposal.value,
							));

							Self::deposit_event(Event::Awarded {
								proposal_index: proposal.index,
								award: proposal.value,
								account: proposal.beneficiary.clone(),
							});

							to_remove.push(idx);
						} else {
							missed_any = true;
						}
					}
				}

				// Remove processed proposals
				for idx in &to_remove {
					proposals.swap_remove(*idx);
				}

				// Update storage if proposals changed
				if !to_remove.is_empty() {
					Managers::<T>::insert(&manager, funding, proposals);
					Some(to_remove.len() as u32)
				} else {
					None
				}
			})
			.sum::<u32>();

		total_weight += T::WeightInfo::on_initialize_proposals(processed_proposals);

		// Existing burn logic remains the same
		if !missed_any && !T::Burn::get().is_zero() {
			let one_minus_burn = T::Burn::get().left_from_one();
			let percent_left =
				one_minus_burn.saturating_pow(spend_periods_passed.unique_saturated_into());
			let new_budget_remaining = percent_left * budget_remaining;
			let burn = budget_remaining.saturating_sub(new_budget_remaining);
			budget_remaining = new_budget_remaining;

			let (debit, credit) = T::Currency::pair(burn);
			imbalance.subsume(debit);
			T::BurnDestination::on_unbalanced(credit);
			Self::deposit_event(Event::Burnt { burnt_funds: burn });
		}

		// Settle balances
		if let Err(problem) =
			T::Currency::settle(&account_id, imbalance, WithdrawReasons::TRANSFER, KeepAlive)
		{
			print("Inconsistent state - couldn't settle imbalance for funds spent by treasury");
			// Nothing else to do here.
			drop(problem);
		}

		Self::deposit_event(Event::Rollover { rollover_balance: budget_remaining });
		total_weight
	}
}

/// TypedGet implementation to get the AccountId of this pallet.
pub struct ProjectAccountId<R>(PhantomData<R>);
impl<R> sp_runtime::traits::TypedGet for ProjectAccountId<R>
where
	R: crate::Config,
{
	type Type = <R as frame_system::Config>::AccountId;
	fn get() -> Self::Type {
		crate::Pallet::<R>::account_id()
	}
}
