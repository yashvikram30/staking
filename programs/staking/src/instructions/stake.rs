use anchor_lang::prelude::*;
use anchor_spl::{
    metadata::{
        mpl_token_metadata::instructions::{
            FreezeDelegatedAccountCpi,
            FreezeDelegatedAccountCpiAccounts,
        },
        MasterEditionAccount,
        Metadata,
        MetadataAccount,
    },
    token::{ approve, Approve, Mint, Token, TokenAccount },
};
use crate::{ state::{ StakeAccount, StakeConfig, UserAccount }, user_account, error::StakeError };

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    pub mint: Account<'info, Mint>,

    pub collection_mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = user,
    )]
    pub user_mint_ata: Account<'info, TokenAccount>,

    /*
    In the Metaplex Token Metadata program, 
    every NFT’s on‑chain metadata is stored in a PDA (Program‑Derived Address) 
    that’s computed according to a fixed scheme. Anchor lets you mirror that derivation 
    in your #[derive(Accounts)] by specifying the exact same seeds.
     */
    #[account(
        seeds = [b"metadata", metadata_program.key().as_ref(), mint.key().as_ref()],
        seeds::program = metadata_program.key(),
        bump,
        constraint = metadata.collection.as_ref().unwrap().key.as_ref() ==
        collection_mint.key().as_ref(),
        constraint = metadata.collection.as_ref().unwrap().verified == true
    )]
    pub metadata: Account<'info, MetadataAccount>,

    /*
    	Confirms this is a “v1” NFT (MasterEdition) so freezing semantics apply correctly
     */
    #[account(
        seeds = [b"metadata", metadata_program.key().as_ref(), mint.key().as_ref(), b"edition"],
        seeds::program = metadata_program.key(),
        bump
    )]
    pub edition: Account<'info, MasterEditionAccount>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, StakeConfig>,

    #[account(
        mut,
        seeds = [b"user",user.key().as_ref()],
        bump = user_account.bump
    )]
    pub user_account: Account<'info, UserAccount>,

    #[account(
        init,
        payer = user,
        space = 8 + StakeAccount::INIT_SPACE,
        seeds = [b"stake", mint.key().as_ref(), config.key().as_ref()],
        bump
    )]
    pub stake_account: Account<'info, StakeAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
}

impl<'info> Stake<'info> {
    pub fn stake(&mut self, bumps: &StakeBumps) -> Result<()> {
        require!(
            self.user_account.amount_staked < self.config.max_stake,
            StakeError::MaxStakeReached
        );

        let cpi_program = self.token_program.to_account_info();

        /*
            what approve is doing - 
            Let stake_account spend 1 token from user_mint_ata, which is owned by user.
         */
        let cpi_accounts = Approve {
            to: self.user_mint_ata.to_account_info(), // token account owned by the user
            delegate: self.stake_account.to_account_info(), //stake account will be the delegate
            authority: self.user.to_account_info(), // user is the authority / owner
        };

        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        approve(cpi_ctx, 1)?;

        let delegate = &self.stake_account.to_account_info();
        let token_account = &self.user_mint_ata.to_account_info();
        let edition = &self.edition.to_account_info();
        let mint = &self.mint.to_account_info();
        let token_program = &self.token_program.to_account_info();
        let metadata_program = &self.metadata_program.to_account_info();

        /*
            After you’ve approved your stake_account as a delegate for the user’s token account,
             you need the Metadata program (the on‑chain MPL Token Metadata program) to actually 
             freeze that delegated account.
             - the token in user_mint_ata becomes non transferrable (locked)
             - the metadata program marks it as "frozen" under the delegate's authority
         */
        FreezeDelegatedAccountCpi::new(metadata_program, FreezeDelegatedAccountCpiAccounts {
            delegate, // stake account pda
            token_account, // the user's ATA holding the nft

            //the MasterEdition PDA (needed by the metadata program to ensure it’s an NFT edition)
            edition, // the NFT'S master edition PDA
            mint, // the NFT's mint
            token_program, // the SPL token program
        }).invoke()?;

        self.stake_account.set_inner(StakeAccount {
            owner: self.user.key(),
            mint: self.mint.key(),
            staked_at: Clock::get()?.unix_timestamp,
            bump: bumps.stake_account,
        });

        self.user_account.amount_staked += 1;

        Ok(())
    }
}