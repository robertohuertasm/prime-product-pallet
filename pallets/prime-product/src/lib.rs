#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, LockIdentifier, LockableCurrency, WithdrawReasons},
	PalletId,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_runtime::traits::AccountIdConversion;
use sp_std::prelude::*;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Problem<T: Config> {
	pub author: <T as frame_system::Config>::AccountId,
	pub number: u32,
	pub prize: BalanceOf<T>,
	pub solved: bool,
}

#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct Solution<T: Config> {
	pub author: <T as frame_system::Config>::AccountId,
	pub number: u32,
	pub factors: (u32, u32),
}

const LOCK_ID: LockIdentifier = *b"12345678";
const PALLET_ID: PalletId = PalletId(*b"primepro");

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: LockableCurrency<Self::AccountId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when a problem is submitted. [who, number, prize]
		ProblemSubmitted(T::AccountId, u32, BalanceOf<T>),
		/// Event emitted when a solution is provided. [who, number, factors, 80% prize]
		SolutionSubmitted(T::AccountId, u32, (u32, u32), BalanceOf<T>),
		// Event emitted when pot information is asked.
		PotInfo(BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The problem has already been submitted.
		ProblemAlreadySubmitted,
		/// The problem has been already solved.
		ProblemAlreadySolved,
		/// An author can only sent one unsolved problem
		AuthorAlreadySubmittedUnsolvedProblem,
		/// The solution is incorrect.
		IncorrectSolution,
		/// The problem doesn't exist.
		ProblemNotFound,
		/// The solution is submitted by the problem's authoer
		SameAuthor,
		/// Not enough funds to pay prize
		NotEnoughFunds,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn problems)]
	pub(super) type Problems<T: Config> = StorageMap<_, Twox64Concat, u32, Problem<T>>;

	#[pallet::storage]
	#[pallet::getter(fn solutions)]
	pub(super) type Solutions<T: Config> = StorageMap<_, Twox64Concat, u32, Solution<T>>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			PALLET_ID.into_account()
		}

		fn check_prime(num: u32) -> bool {
			if num < 2 {
				return false;
			}
			let mut i = 2;
			while i * i <= num {
				if num % i == 0 {
					return false;
				}
				i += 1;
			}
			true
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn submit_problem(
			origin: OriginFor<T>,
			number: u32,
			prize: BalanceOf<T>,
		) -> DispatchResult {
			let author = ensure_signed(origin)?;

			// does problem already exist? We don't support several prizes for the same problem for the moment
			ensure!(Self::problems(number).is_none(), <Error<T>>::ProblemAlreadySubmitted);

			// to make things easier for us, we don't allow an author to submit more than one unsolved problem.
			let more_than_one_problem_sent_by_same_author =
				<Problems<T>>::iter().any(|(_, p)| !p.solved && p.author == author);
			ensure!(
				!more_than_one_problem_sent_by_same_author,
				<Error<T>>::AuthorAlreadySubmittedUnsolvedProblem
			);

			// does the author has enough funds?
			let author_free_balance = T::Currency::free_balance(&author);
			ensure!(author_free_balance >= prize, <Error<T>>::NotEnoughFunds);

			// block the author's funds from being spent
			T::Currency::set_lock(LOCK_ID, &author, prize, WithdrawReasons::all());

			// store the problem
			let problem = Problem { author: author.clone(), solved: false, prize, number };
			<Problems<T>>::insert(number, problem);

			// emit event
			Self::deposit_event(Event::ProblemSubmitted(author, number, prize));

			Ok(())
		}

		#[pallet::weight(10)]
		#[frame_support::transactional]
		pub fn submit_solution(
			origin: OriginFor<T>,
			number: u32,
			factor1: u32,
			factor2: u32,
		) -> DispatchResult {
			let solution_author = ensure_signed(origin)?;

			// does problem exist?
			let problem = Self::problems(number).ok_or(<Error<T>>::ProblemNotFound)?;

			// is the problem solved?
			ensure!(!problem.solved, <Error<T>>::ProblemAlreadySolved);

			// is the problem author the same as the solution author?
			ensure!(problem.author != solution_author, <Error<T>>::SameAuthor);

			// is solution correct?
			let is_correct = {
				factor1 * factor2 == number
					&& Self::check_prime(factor1)
					&& Self::check_prime(factor2)
			};

			ensure!(is_correct, <Error<T>>::IncorrectSolution);

			let factors = (factor1, factor2);

			// store the solution
			<Solutions<T>>::insert(
				number,
				Solution { author: solution_author.clone(), factors, number },
			);

			// modify the problem -- set to correct
			<Problems<T>>::mutate(number, |problem| {
				problem.as_mut().map(|p| p.solved = true);
			});

			// unlock the funds
			T::Currency::remove_lock(LOCK_ID, &problem.author);

			//ONLY PAY 80% and REMAINING 20% to pallet's treasury
			let percent_80_of_prize =
				<BalanceOf<T>>::from(80u32) * problem.prize / <BalanceOf<T>>::from(100u32);

			let percent_20_of_prize = problem.prize - percent_80_of_prize;

			// pay to the solution author
			T::Currency::transfer(
				&problem.author,
				&solution_author,
				percent_80_of_prize,
				ExistenceRequirement::KeepAlive,
			)?;

			// pay to the treasury pool
			T::Currency::transfer(
				&problem.author,
				&Self::account_id(),
				percent_20_of_prize,
				ExistenceRequirement::KeepAlive,
			)?;

			// emit event
			Self::deposit_event(Event::SolutionSubmitted(
				solution_author,
				number,
				factors,
				percent_80_of_prize,
			));

			Ok(())
		}

		#[pallet::weight(1)]
		pub fn pot(_origin: OriginFor<T>) -> DispatchResult {
			let balance = T::Currency::free_balance(&Self::account_id());
			Self::deposit_event(Event::PotInfo(balance));
			Ok(())
		}
	}
}
