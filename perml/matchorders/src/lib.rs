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
extern crate perml_tokens as tokens;
extern crate perml_pendingorders as pendingorders;

use support::{decl_module, decl_storage, decl_event, StorageValue, StorageMap, dispatch::Result};
use runtime_primitives::traits::{OnFinalize, Zero};
use system::ensure_signed;
use sr_std::prelude::*;

type Symbol = tokens::Symbol;
type TokenBalanceOf<T> = <T as tokens::Trait>::TokenBalance;
type OrderId<T> = <T as pendingorders::Trait>::OrderId;
type PriceType<T> = <T as pendingorders::Trait>::PriceType;
type OrderType = pendingorders::OrderType;
type Filled<T> = pendingorders::Filled<T>;
type Order<T> = pendingorders::Order<T>;

pub trait Trait: tokens::Trait + pendingorders::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

// TODO
// 改变pendingorders状态

decl_storage! {
	trait Store for Module<T: Trait> as matchorders {
		Something get(something): Option<u32>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		fn match_orders() -> Result {
			Self::do_match_orders()
		}

		fn on_finalize(time: T::BlockNumber) {
			Self::match_orders();
		}
	}
}

impl<T: Trait> Module<T> {

	//匹配
	fn do_match_orders() -> Result {
		// 循环所有交易对
		let symbol_pair_vec: Vec<(Symbol, Symbol)> = <tokens::Module<T>>::symbol_pairs(&1u32);
    for symbol_pair in symbol_pair_vec {
			let sym0 = symbol_pair.0;
			let sym1 = symbol_pair.1;
			let buy_key = (sym0.clone(), sym1.clone(), OrderType::Buy);
			let sell_key = (sym0.clone(), sym1.clone(), OrderType::Sell);

			// 买单价格列表
			let all_buyers_prices: Vec<PriceType<T>> = <pendingorders::Module<T>>::price_list(buy_key).0.keys().cloned().collect();
      // 卖单价格列表
			let all_sellers_prices: Vec<PriceType<T>> = <pendingorders::Module<T>>::price_list(sell_key).0.keys().cloned().collect();

			Self::match_buyers(all_buyers_prices, all_sellers_prices, sym0.clone(), sym1.clone());
		}
		Ok(())
	}

  // 匹配所有买单
	fn match_buyers(buyer_prices: Vec<PriceType<T>>, seller_prices: Vec<PriceType<T>>, sym0: Symbol, sym1: Symbol) {
		// 买单按价格从高到低匹配
		'buy_loop: for buyer_price in buyer_prices.iter().rev() {
			// 卖单按价格从低到高匹配
      for (i, seller_price) in seller_prices.iter().enumerate() {
				if seller_price > buyer_price {
						break;
				}
				let finish_the_buyer = Self::match_one_buyer_price(buyer_price.clone(), seller_price.clone(), sym0.clone(), sym1.clone());
        if finish_the_buyer {
					continue 'buy_loop;
				}
			}
		}
	}

  // 匹配一个价格的所有买单
	fn match_one_buyer_price(buyer_price: PriceType<T>, seller_price: PriceType<T>, sym0: Symbol, sym1: Symbol) -> bool {
		let buyer_order_id_map_key = (sym0.clone(), sym1.clone(), OrderType::Buy, buyer_price);
		let mut buyer_bn_btreemap = <pendingorders::Module<T>>::order_id_map(buyer_order_id_map_key);
		let buyer_block_numbers: Vec<T::BlockNumber> = buyer_bn_btreemap.0.keys().cloned().collect();

		let mut finish = false;
		for buyer_bn in buyer_block_numbers.iter() {
			let buyer_order_ids: Vec<T::OrderId> = buyer_bn_btreemap.0.get(buyer_bn).unwrap_or(&Vec::new()).to_vec();
			for buyer_order_id in buyer_order_ids.iter() {
				finish = Self::match_one_buyer_order_id(buyer_order_id.clone(), seller_price.clone(), sym0.clone(), sym1.clone());
				if finish {
					break
				}
			}
		}

		return finish;
	}

	// 匹配一个买单
	fn match_one_buyer_order_id(buyer_order_id: T::OrderId, seller_price: PriceType<T>, sym0: Symbol, sym1: Symbol) -> bool {

		let seller_order_id_map_key = (sym0.clone(), sym1.clone(), OrderType::Sell, seller_price);
		let seller_bn_btreemap = <pendingorders::Module<T>>::order_id_map(seller_order_id_map_key);
		let seller_block_numbers: Vec<T::BlockNumber> = seller_bn_btreemap.0.keys().cloned().collect();

		for seller_bn in seller_block_numbers.iter() {
			let seller_order_ids: Vec<T::OrderId> = seller_bn_btreemap.0.get(seller_bn).unwrap_or(&Vec::new()).to_vec();
			for seller_order_id in seller_order_ids.iter() {
				let finish = Self::match_one_by_one(buyer_order_id.clone(), seller_order_id.clone(), sym0.clone(), sym1.clone());
				if finish {
					return true;
				}
			}
		}

		return false;
	}

	// 匹配一个买单和一个卖单
	fn match_one_by_one(buyer_order_id: T::OrderId, seller_order_id: T::OrderId, sym0: Symbol, sym1: Symbol) -> bool {
		let mut buyer_order = match <pendingorders::Module<T>>::order_map(&buyer_order_id) {
			Some(order)  => order,
			None => return true
		};

		let buyer_price = buyer_order.price;
		let buyer_remain_amount = buyer_order.total - buyer_order.total_filled;

		let mut seller_order = match <pendingorders::Module<T>>::order_map(&seller_order_id) {
			Some(order)  => order,
			None => return false
		};

		let seller_price = seller_order.price;
		let seller_remain_amount = seller_order.total - seller_order.total_filled;

		let mut update_amount: TokenBalanceOf<T> = Zero::zero();
		let mut finish:bool = false;
		if buyer_remain_amount <= seller_remain_amount {
			update_amount = buyer_remain_amount;
			finish = true;
		} else {
			finish = false;
		}
//		seller_order.update(update_amount.clone(), buyer_price.clone());
//		<pendingorders::OrderMap<T>>::insert(seller_order_id, seller_order);
//		buyer_order.update(update_amount.clone(), buyer_price.clone());
//		<pendingorders::OrderMap<T>>::insert(buyer_order_id, buyer_order);
		// TODO
		// transfer

		return finish;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as system::Trait>::BlockNumber,
		<T as pendingorders::Trait>::OrderId,
		<T as pendingorders::Trait>::PriceType,
		<T as tokens::Trait>::TokenBalance,
	{
		// 匹配事件
		Match(AccountId, OrderId, Symbol, Symbol, PriceType, TokenBalance, BlockNumber),
	}
);


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
	type matchorders = Module<Test>;

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
			assert_ok!(matchorders::do_something(Origin::signed(1), 42));
			// asserting that the stored value is equal to what we stored
			assert_eq!(matchorders::something(), Some(42));
		});
	}
}
