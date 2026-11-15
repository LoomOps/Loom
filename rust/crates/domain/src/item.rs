//! Lossless common identity. Allocation is an application concern; this module has no I/O.
use crate::{DomainError, Result};
use std::{fmt, str::FromStr};
macro_rules! id {
    ($($name:ident),+) => {$ (
        #[derive(Clone,Copy,Debug,Eq,PartialEq,Ord,PartialOrd,Hash)]
        pub struct $name([u8;16]);
        impl $name {
            pub fn from_bytes(bytes:[u8;16])->Self {Self(bytes)}
            pub fn bytes(self)->[u8;16] {self.0}
            pub fn parse(text:&str)->Result<Self> {
                if text.is_empty() || text.len()>39 || text.starts_with('0') || !text.bytes().all(|b|b.is_ascii_digit()) {return Err(DomainError("Noncanonical Item ID"));}
                let n=u128::from_str(text).map_err(|_|DomainError("Item ID overflow"))?;
                Ok(Self(n.to_be_bytes()))
            }
        }
        impl fmt::Display for $name {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {write!(f,"{}",u128::from_be_bytes(self.0))}}
    )+};
}
id!(ItemId, RevisionId, FileId, StreamId, LocatorId);
