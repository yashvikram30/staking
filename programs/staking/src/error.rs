use anchor_lang::prelude::*;

#[error_code]
pub enum StakeError {
    #[msg("You are staking more than the max amount")]
    MaxStakeReached,
}
