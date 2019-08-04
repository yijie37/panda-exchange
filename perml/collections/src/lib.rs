//#![cfg_attr(not(feature = "std"), no_std)]
/// A runtime module template with necessary imports

/// Feel free to remove or edit this file as needed.
/// If you change the name of this file, make sure to update its references in runtime/src/lib.rs
/// If you remove this file, you can remove those references


/// For more guidance on Substrate modules, see the example module
/// https://github.com/paritytech/substrate/blob/master/srml/example/src/lib.rs

//extern crate parity_codec;

//use sr_std::prelude::*;
extern crate srml_support as support;
extern crate parity_codec as codec;

use codec::{Decode, Encode, Input, Output};
use support::rstd::collections::btree_map::BTreeMap;

#[derive(Default)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct CodecBTreeMap<K: Ord, V>(
  pub BTreeMap<K, V>
);

impl<K: Encode + Ord, V: Encode> Encode for CodecBTreeMap<K, V> {
  fn encode_to<W: Output>(&self, dest: &mut W) {
    let len = self.0.len();
    assert!(
      len <= u32::max_value() as usize,
      "Attempted to serialize a collection with too many elements."
    );
    (len as u32).encode_to(dest);
    for i in self.0.iter() {
      i.encode_to(dest);
    }
  }
}

impl<K: Decode + Ord, V: Decode> Decode for CodecBTreeMap<K, V> {
  fn decode<I: Input>(input: &mut I) -> Option<Self> {
    u32::decode(input).and_then(move |len| {
      let mut r: BTreeMap<K, V> = BTreeMap::new(); // with_capacity(len as usize);
      for _ in 0..len {
        let (key, v) = <(K, V)>::decode(input)?;
        r.insert(key, v);
      }
      Some(CodecBTreeMap::<K, V>(r))
    })
  }
}


