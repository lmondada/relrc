//! Unique hash-based identifiers for [`RelRc`] objects.

use std::hash::Hash;

use fxhash::hash;

use derive_more::{From, Into};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{detached::DetachedInnerData, node::InnerData};

/// A unique hash-based identifier for [`RelRc`] objects.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, From, Into, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RelRcHash(usize);

impl<N: Hash, E: Hash> From<&InnerData<N, E>> for RelRcHash {
    fn from(obj: &InnerData<N, E>) -> Self {
        let detached = DetachedInnerData::from(obj);
        RelRcHash(hash(&detached))
    }
}
