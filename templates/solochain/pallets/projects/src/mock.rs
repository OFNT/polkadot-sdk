use crate as pallet_projects;
use frame_support::{derive_impl, parameter_types, PalletId};
use sp_runtime::{
	traits::{ConstU32, ConstU64},
	BuildStorage, Permill,
};

type Block = frame_system::mocking::MockBlock<Test>;

#[frame_support::runtime]
mod runtime {
	use frame_system::pallet;

	// The main runtime
	#[runtime::runtime]
	// Runtime Types to be generated
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask,
		RuntimeViewFunction
	)]
	pub struct Test;

	#[runtime::pallet_index(0)]
	pub type Balances = pallet_balances::Pallet<Test>;

	#[runtime::pallet_index(1)]
	pub type System = frame_system::Pallet<Test>;

	#[runtime::pallet_index(2)]
	pub type Template = pallet_projects::Pallet<Test>;
}

pub struct TestOrigin;
impl frame_support::traits::EnsureOrigin<RuntimeOrigin> for TestOrigin {
	type Success = u64;
	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Result::<frame_system::RawOrigin<_>, RuntimeOrigin>::from(o).and_then(|o| match o {
			frame_system::RawOrigin::Root => Ok(u64::max_value()),
			frame_system::RawOrigin::Signed(10) => Ok(5),
			frame_system::RawOrigin::Signed(11) => Ok(10),
			frame_system::RawOrigin::Signed(12) => Ok(20),
			frame_system::RawOrigin::Signed(13) => Ok(50),
			frame_system::RawOrigin::Signed(14) => Ok(500),
			r => Err(RuntimeOrigin::from(r)),
		})
	}
	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		if TEST_SPEND_ORIGIN_TRY_SUCCESFUL_ORIGIN_ERR.with(|i| *i.borrow()) {
			Err(())
		} else {
			Ok(frame_system::RawOrigin::Root.into())
		}
	}
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountData = pallet_balances::AccountData<u64>;
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(50);
	pub const ProjectsPalletId: PalletId = PalletId(*b"py/trsry");
	pub const SpendPayoutPeriod: u64 = 5;
	pub const Burn: Permill = Permill::from_percent(50);
}

impl pallet_projects::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Currency = pallet_balances::Pallet<Test>;
	type AdminOrigin = TestOrigin;
	type MaxProposalsPerManager = ConstU32<100>;
	type SpendPeriod = ConstU64<2>;
	type Burn = Burn;
	type PalletId = ProjectsPalletId;
	type ProposalBond = ProposalBond;
	type BurnDestination = ();
	type SpendFunds = ();
	type MaxApprovals = ConstU32<100>;
	type SpendOrigin = TestOrigin;
	type Beneficiary = u128;
	type BlockNumberProvider = System;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::<Test>::default().build_storage().unwrap().into()
}
