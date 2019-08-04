#![cfg_attr(not(feature = "std"), no_std)]
/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

extern crate perml_tokens as tokens;
extern crate srml_support as support;
extern crate sr_primitives as runtime_primitives;
extern crate parity_codec;
extern crate srml_system as system;
extern crate sr_std;
extern crate perml_collections;

use support::{decl_module, decl_storage, decl_event, StorageMap, StorageValue, dispatch::Result, Parameter, rstd};
use runtime_primitives::traits::{SimpleArithmetic, Bounded, One, CheckedAdd, CheckedSub};
use parity_codec::{Encode, Decode};
use system::{ensure_signed, RawOrigin};
use tokens::{Symbol, Token};
use sr_std::prelude::*;
use rstd::result;
use rstd::collections::btree_map::BTreeMap;
use perml_collections::CodecBTreeMap;
use std::cmp::Ordering;
//use parity_codec::alloc::collections::BTreeMap;

#[derive(Encode, Decode, PartialEq, Eq, Clone, Debug)]
pub enum OrderType {
  Buy = 0,
  Sell = 1,
}

impl Ord for OrderType {
  fn cmp(&self, other: &Self) -> Ordering {
    self.cmp(other)
  }
}

impl PartialOrd for OrderType {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

pub trait Trait: tokens::Trait {
  type OrderId: Parameter + Default + Bounded + SimpleArithmetic;
  type PriceType: Parameter + Default + Bounded + SimpleArithmetic;
  type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type TokenBalanceOf<T> = <T as tokens::Trait>::TokenBalance;
type BlockNumber<T> = <T as system::Trait>::BlockNumber;

#[derive(Encode, Decode, Default, PartialEq, Clone)]
pub struct Filled<T: Trait> {
  pub price: T::PriceType,
  // 成交价格
  pub amount: <T as tokens::Trait>::TokenBalance,
  // 成交数量
  pub block_number: u32, // 成交区块号
}

#[derive(Encode, Decode, PartialEq, Clone)]
pub struct Order<T: Trait> {
  pub id: T::OrderId,
  // 挂单id
  pub acc: T::AccountId,
  // 用户账号
  pub sym0: Symbol,
  // 交易对的第一个token
  pub sym1: Symbol,
  // 交易对的第二个token
  pub side: OrderType,
  // 买/卖
  pub price: T::PriceType,
  // 挂单价格
  pub total: <T as tokens::Trait>::TokenBalance,
  // 总挂单数量
  pub total_filled: u32,
  // 总成交数量
  pub fills: Vec<Filled<T>>,
  // 成交数组
  pub block_number: BlockNumber<T>, // 挂单区块号
}

decl_storage! {
	trait Store for Module<T: Trait> as pendingorders {
		/// 每个挂单的唯一id
		pub OrderSeq get(order_id): T::OrderId;

		/// (交易对, 交易方向) => 有序挂单价
		pub PriceList get(price_list): map (Symbol, Symbol, OrderType) => CodecBTreeMap<T::PriceType, ()>;

		/// (交易对, 交易方向，挂单价） => blocknum有序的挂单id列表
		pub OrderIdMap get(order_id_map): map(Symbol, Symbol, OrderType, T::PriceType) => CodecBTreeMap<BlockNumber<T>, Vec<T::OrderId>>;

		/// order_id => Order
		pub OrderMap get(order_map): map T::OrderId => Option<Order<T>>;

		/// OrderId 属于哪个用户
		pub OrderOf get(order_of): map T::OrderId => T::AccountId;

		/// 某个用户的所有order
		pub Orders get(orders): map T::AccountId => Vec<T::OrderId>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event<T>() = default;

		/// 挂单
		pub fn order(origin,
								 sym0: Symbol,
								 sym1: Symbol,
								 price: T::PriceType,
								 amount: TokenBalanceOf<T>,
								 side: OrderType
								) -> Result
		{
			Self::do_order(origin, sym0, sym1, price, amount, side)
		}

		/// 取消挂单
		pub fn cancel_order(origin, order_id: T::OrderId) -> Result {
			Self::do_cancel_order(origin, order_id)
		}

	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as self::Trait>::OrderId,
		<T as self::Trait>::PriceType,
		TokenBalance = TokenBalanceOf<T>
	{
		/// 挂单事件
		Ordered(AccountId, Symbol, Symbol, PriceType, TokenBalance, OrderType),

		/// 取消挂单事件
		OrderCanceled(AccountId, OrderId),
	}
);

impl<T: Trait> Module<T> {

  fn next_order_id() -> result::Result<T::OrderId, &'static str> {
    let order_id = Self::order_id();
    let next_id = order_id.checked_add(&One::one()).ok_or("Token id overflow")?;
    Ok(next_id)
  }

  fn insert_orderid_to_orders(acc: T::AccountId, order_id: T::OrderId) -> Result{
    /// 某个用户的所有order
    let mut order_vec = Self::orders(&acc);
    order_vec.push(order_id);
    <Orders<T>>::insert(acc, order_vec);
    Ok(())
  }

  fn do_order(origin: T::Origin,
              sym0: Symbol,
              sym1: Symbol,
              price: T::PriceType,
              amount: TokenBalanceOf<T>,
              side: OrderType) -> Result
  {
    let sender = ensure_signed(origin)?;
//    let new_origin: T::Origin = RawOrigin::from(Some(sender.clone()));
    let new_origin: T::Origin = RawOrigin::Signed(sender.clone()).into();

    /// 检查交易对是否已注册
    let symbols = (sym0.clone(), sym1.clone());
    <tokens::Module<T>>::symbol_pairs(symbols).ok_or("Symbol pair not registered")?;

    /// 检查余额是否足够
    let token_key = (sender.clone(), sym1.clone());
    let free_token = <tokens::Module<T>>::free_token(token_key);
    assert!(free_token > amount, "Insufficient tokens to order");

    /// 构造order
    let order_id = Self::next_order_id()?;
    let filled_vec:Vec<Filled<T>> = Vec::new();
    let block_num = <system::Module<T>>::block_number();
    let order: Order<T> = Order {
      id: order_id.clone(),
      acc: sender.clone(),
      sym0: sym0.clone(),
      sym1: sym1.clone(),
      side: side.clone(),
      price: price.clone(),
      total: amount.clone(),
      total_filled: 0u32,
      fills: filled_vec,
      block_number: block_num.clone()
    };

    /// 冻结资金
    <tokens::Module<T>>::freeze(new_origin, sender.clone(), sym1.clone(), amount.clone())?;

    /// 检查price
    let price_list_key = (sym0.clone(), sym1.clone(), side.clone());
    let prices = Self::price_list(price_list_key.clone());
    let order_id_map_key = (sym0.clone(), sym1.clone(), side.clone(), price.clone());
    if prices.0.contains_key(&price) {
      // 已有当前报价的挂单

      // 当前block number的所有order_id列表
      let mut order_ids_by_bn = Self::order_id_map(&order_id_map_key);
      let tmp_vec: Vec<T::OrderId> = Vec::new();
      let mut order_ids_vec = order_ids_by_bn.0.get(&block_num).unwrap_or(&tmp_vec).clone();

      // 插入当前order
      order_ids_vec.push(order_id.clone());

      // 插入OrderIdMap
      order_ids_by_bn.0.insert(block_num.clone(), order_ids_vec);
      <OrderIdMap<T>>::insert(&order_id_map_key, order_ids_by_bn);
    } else {
      // 没有当前报价的挂单

      // 插入当前报价
      let mut btm: BTreeMap<T::PriceType, ()> = BTreeMap::new();
      btm.insert(price.clone(), ());
      let cbtm = CodecBTreeMap(btm);
      <PriceList<T>>::insert(price_list_key.clone(), cbtm);

      // 插入OrderIdMap
      let mut order_id_vec:Vec<T::OrderId> = Vec::new();
      order_id_vec.push(order_id.clone());
      let mut btm: BTreeMap<BlockNumber<T>, Vec<T::OrderId>> = BTreeMap::new();
      btm.insert(block_num.clone(), order_id_vec);
      let cbtm = CodecBTreeMap(btm);
      <OrderIdMap<T>>::insert(&order_id_map_key, cbtm);
    }

    // 当前order插入OrderMap
    <OrderMap<T>>::insert(order_id.clone(), order);

    // 当前order插入用户的orders
    Self::insert_orderid_to_orders(sender.clone(), order_id.clone());


    Self::deposit_event(RawEvent::Ordered(sender, sym0, sym1, price, amount, side));

    Ok(())
  }

  fn do_cancel_order(origin: T::Origin, order_id: T::OrderId) -> Result {

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
		testing::{Digest, DigestItem, Header},
	};

  impl_outer_origin! {
		pub enum Origin for Test {}
	}

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

  type pendingorders = Module<Test>;

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
      assert_ok!(pendingorders::do_something(Origin::signed(1), 42));
      // asserting that the stored value is equal to what we stored
      assert_eq!(pendingorders::something(), Some(42));
    });
  }
}
