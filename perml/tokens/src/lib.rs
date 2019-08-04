#![cfg_attr(not(feature = "std"), no_std)]
/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

extern crate srml_support as support;
extern crate sr_primitives as runtime_primitives;
extern crate parity_codec;
extern crate srml_system as system;
extern crate sr_std;


use support::{decl_module, decl_storage, decl_event, StorageMap, StorageValue, dispatch::Result, Parameter};
use runtime_primitives::traits::{SimpleArithmetic, Bounded, One, CheckedAdd, CheckedSub};
use parity_codec::{Encode, Decode};
use system::ensure_signed;
use sr_std::prelude::*;

pub trait Trait: system::Trait {
	// Token id type.
	type TokenId: Parameter + Default + Bounded + SimpleArithmetic;

  // Token balance type.
  type TokenBalance: Parameter + Default + Bounded + SimpleArithmetic + Copy;

	// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

pub type Symbol = Vec<u8>;


#[derive(Encode, Decode, Default, PartialEq, Clone)]
pub struct Token<T: Trait> {
	symbol: Symbol,
	total_supply: T::TokenBalance,
}

decl_storage! {
	trait Store for Module<T: Trait> as Tokens {
		// 每种token有一个唯一id
		pub TokenSeq get(token_id): T::TokenId;

		// Token信息
		pub TokenInfo get(token_info): map T::TokenId => Symbol;

		// 已注册交易对
		pub SymbolPairs get(symbol_pairs): map (Symbol, Symbol) => Option<T::AccountId>;

		// 某个币种，某个AccountId的token balance
		pub BalanceOf get(balance_of): map (T::AccountId, T::TokenId) => T::TokenBalance;

		// 某个币种，某个AccountId的allowance
		pub Allowance get(allowance): map (T::AccountId, T::TokenId) => T::TokenBalance;

		// 某个账号，某个token的 free token
		pub FreeToken get(free_token): map (T::AccountId, Symbol)	=> T::TokenBalance;

		// 某个账号，某个token的 freezed token
		pub FreezedToken get(freezed_token): map (T::AccountId, Symbol) => T::TokenBalance;

		// 某个账号所有的token列表
		pub OwnedTokenList get(owned_token_list): map T::AccountId => Vec<Symbol>;
	}
}

decl_module! {
	// The module declaration.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		// 发行token
		pub fn issue(origin, symbol: Symbol, total_supply: T::TokenBalance)	-> Result {
			Self::do_issue(origin, symbol, total_supply)
		}

		// 注册交易对
//		pub fn register_symbol_pairs(origin, sym0: Symbol, sym1: Symbol) -> Result {
//			Self::do_register_symbol_pairs(origin, sym0, sym1)
//		}

		// 转移token
		pub fn transfer(origin, to: T::AccountId, symbol: Symbol, value: T::TokenBalance) -> Result {
			Self::do_transfer(origin, to, symbol, value)
		}

		// 冻结token
		pub fn freeze(origin, acc: T::AccountId, symbol: Symbol, value: T::TokenBalance) -> Result {
			Self::do_freeze(origin, acc, symbol, value)
		}

		// 解冻token
		pub fn unfreeze(origin, acc: T::AccountId, symbol: Symbol, value: T::TokenBalance) -> Result {
			Self::do_unfreeze(origin, acc, symbol, value)
		}
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as self::Trait>::TokenId,
		<T as self::Trait>::TokenBalance,
	{
		// 发行token
		Issued(TokenId, Symbol, TokenBalance),

		// 注册交易对
		SymbolPairRegisterd(Symbol, Symbol),

		// 转账事件
		Transfered(Symbol, AccountId, AccountId, TokenBalance),

		// approve事件
		Approval(TokenId, AccountId, AccountId, TokenBalance),

		// Freeze事件
		Freezed(AccountId, Symbol, TokenBalance),

		// Unfreeze事件
		Unfreezed(AccountId, Symbol, TokenBalance),
	}
);

impl<T: Trait> Module<T> {
	// 发行token
	fn do_issue(origin: T::Origin, symbol: Symbol, total_supply: T::TokenBalance) -> Result
		{
		let sender = ensure_signed(origin)?;

		let token_id = Self::token_id();
		let next_id = token_id.checked_add(&One::one()).ok_or("Token id overflow")?; //
		<TokenSeq<T>>::put(next_id);

		let token: Token<T> = Token {
			symbol: symbol.clone(),
			total_supply,
		};

		<TokenInfo<T>>::insert(token_id.clone(), symbol.clone());
		<BalanceOf<T>>::insert((sender.clone(), token_id.clone()), total_supply);
		<FreeToken<T>>::insert((sender.clone(), symbol.clone()), total_supply);
		Self::deposit_event(RawEvent::Issued(token_id, symbol, total_supply));

		Ok(())
	}

//	fn do_register_symbol_pairs(origin: T::Origin, sym0: Symbol, sym1: Symbol) -> Result
//	{
//		let sender = ensure_signed(origin)?;
//		let key = (sym0.clone(), sym1.clone());
//    let exists = match Self::symbol_pairs(&key) {
//			Some(u) => return Err("Symbol pair registered."),
//			None => sender.clone(),
//		};
//		<SymbolPairs<T>>::insert(key, Some(&sender));
//		Self::deposit_event(RawEvent::SymbolPairRegisterd(sym0, sym1));
//
//		Ok(())
//	}

	fn do_transfer(origin: T::Origin, to: T::AccountId, symbol: Symbol, value: T::TokenBalance) -> Result {
		let sender = ensure_signed(origin)?;
		let sender_key = (sender.clone(), symbol.clone());
		let reciever_key = (to.clone(), symbol.clone());
		let sender_free_token = Self::freezed_token(&sender_key);
		let reciever_free_token = Self::freezed_token(&reciever_key);
		if sender_free_token < value {
			return Err("Insufficent free token to send");
		}
		let new_sender_free_token = match sender_free_token.checked_sub(&value) {
			Some(t) => t,
			None => return Err("Insufficent free token to send"),
		};
		let new_reciever_free_token = match reciever_free_token.checked_add(&value) {
			Some(t) => t,
			None => return Err("Reciever free token overflowed"),
		};
		<FreeToken<T>>::insert(&sender_key, new_sender_free_token);
		<FreeToken<T>>::insert(&reciever_key, new_reciever_free_token);
		Self::deposit_event(RawEvent::Transfered(symbol, sender, to.clone(), value));

		Ok(())
	}

	fn do_freeze(origin: T::Origin, acc: T::AccountId, symbol: Symbol, value: T::TokenBalance) -> Result {
		let sender = ensure_signed(origin)?;
		assert!(sender == acc, "Can not freeze other's token");

		let key = (acc.clone(), symbol.clone());
		let free_token = Self::free_token(&key);
		let freezed_token = Self::freezed_token(&key);
    let new_free_token = match free_token.checked_sub(&value) {
			Some(t) => t,
			None => return Err("Insufficient free token"),
		};
		let new_freezed_token = match freezed_token.checked_add(&value) {
			Some(t) => t,
			None => return Err("Freezed token add overflowed"),
		};

		<FreeToken<T>>::insert(&key, new_free_token);
		<FreezedToken<T>>::insert(&key, new_freezed_token);
		Self::deposit_event(RawEvent::Freezed(acc, symbol, value));

    Ok(())
	}

	fn do_unfreeze(origin: T::Origin, acc: T::AccountId, symbol: Symbol, value: T::TokenBalance) -> Result {
		let sender = ensure_signed(origin)?;
		assert!(sender == acc, "Can not unfreeze other's token");

		let key = (acc.clone(), symbol.clone());
		let free_token = Self::free_token(&key);
		let freezed_token = Self::freezed_token(&key);
		let new_free_token = match free_token.checked_add(&value) {
			Some(t) => t,
			None => return Err("Free token add overflow"),
		};
		let new_freezed_token = match freezed_token.checked_sub(&value) {
			Some(t) => t,
			None => return Err("Insufficient freezed token"),
		};

		<FreeToken<T>>::insert(&key, new_free_token);
		<FreezedToken<T>>::insert(&key, new_freezed_token);
		Self::deposit_event(RawEvent::Freezed(acc, symbol, value));

		Ok(())
	}



}

/// tests for this module
#[cfg(test)]
mod tests {
	use super::*;

	use runtime_io::with_externalities;
	use primitives::{H256, Blake2Hasher};
	use support::{impl_outer_origin, assert_ok};
	use runtime_primitives::{
		BuildStorage,
		traits::{BlakeTwo256, IdentityLookup},
		testing::{Digest, DigestItem, Header}
	};

	impl_outer_origin! {
		pub enum Origin for Test {}
	}

	// For testing the module, we construct most of a mock runtime. This means
	// first constructing a configuration type (`Test`) which `impl`s each of the
	// configuration traits of modules we want to use.
	#[derive(Clone, Eq, PartialEq)]
	pub struct Test;
	impl system::Trait for Test {
		type Origin = Origin;
		type Index = u64;
		type BlockNumber = u64;
		type Hash = H256;
		type Hashing = BlakeTwo256;
		type Digest = Digest;
		type AccountId = u64;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Header = Header;
		type Event = ();
		type Log = DigestItem;
	}
	impl Trait for Test {
		type Event = ();
	}
	type token = Module<Test>;

	// This function basically just builds a genesis storage key/value store according to
	// our desired mockup.
	fn new_test_ext() -> runtime_io::TestExternalities<Blake2Hasher> {
		system::GenesisConfig::<Test>::default().build_storage().unwrap().0.into()
	}

	#[test]
	fn it_works_for_default_value() {
		with_externalities(&mut new_test_ext(), || {
			// Just a dummy test for the dummy funtion `do_something`
			// calling the `do_something` function with a value 42
			assert_ok!(token::do_something(Origin::signed(1), 42));
			// asserting that the stored value is equal to what we stored
			assert_eq!(token::something(), Some(42));
		});
	}
}
