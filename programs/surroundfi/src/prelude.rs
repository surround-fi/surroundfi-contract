use anchor_lang::prelude::*;

pub type SurroundfiResult<G = ()> = Result<G>;

pub use crate::{errors::SurroundfiError, state::surroundfi_group::SurroundfiGroup};