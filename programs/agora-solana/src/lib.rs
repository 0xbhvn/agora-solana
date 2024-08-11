use anchor_lang::prelude::*;
use anchor_lang::solana_program::clock::Clock;

declare_id!("Dq38DoFThxyXXrgz57DNvL8iCAgQyKwJ88fNGKWZpGzY");

#[program]
pub mod agora_governor {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        voting_delay: u64,
        voting_period: u64,
        proposal_threshold: u64,
    ) -> Result<()> {
        let governor = &mut ctx.accounts.governor;
        governor.admin = *ctx.accounts.admin.key;
        governor.manager = *ctx.accounts.manager.key;
        governor.voting_delay = voting_delay;
        governor.voting_period = voting_period;
        governor.proposal_threshold = proposal_threshold;
        governor.proposal_count = 0;
        Ok(())
    }

    pub fn create_proposal(
        ctx: Context<CreateProposal>,
        description: String,
        proposal_type: u8,
    ) -> Result<()> {
        let governor = &mut ctx.accounts.governor;
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;

        require!(
            governor.get_votes(&ctx.accounts.proposer.key(), clock.slot) >= governor.proposal_threshold
                || ctx.accounts.proposer.key() == &governor.manager,
            GovernorError::InsufficientProposerVotes
        );

        let proposal_type_info = governor.proposal_types.get(&proposal_type).ok_or(GovernorError::InvalidProposalType)?;

        proposal.id = governor.proposal_count;
        proposal.proposer = *ctx.accounts.proposer.key;
        proposal.description = description;
        proposal.proposal_type = proposal_type;
        proposal.start_block = clock.slot + governor.voting_delay;
        proposal.end_block = proposal.start_block + governor.voting_period;
        proposal.executed = false;
        proposal.canceled = false;

        governor.proposal_count += 1;

        emit!(ProposalCreated {
            proposal_id: proposal.id,
            proposer: proposal.proposer,
            start_block: proposal.start_block,
            end_block: proposal.end_block,
            description: proposal.description.clone(),
            proposal_type,
        });

        Ok(())
    }

    pub fn cast_vote(
        ctx: Context<CastVote>,
        proposal_id: u64,
        support: bool,
    ) -> Result<()> {
        let governor = &ctx.accounts.governor;
        let proposal = &mut ctx.accounts.proposal;
        let vote = &mut ctx.accounts.vote;
        let clock = Clock::get()?;

        require!(
            clock.slot >= proposal.start_block && clock.slot <= proposal.end_block,
            GovernorError::VotingPeriodInactive
        );

        let voter_weight = governor.get_votes(&ctx.accounts.voter.key(), proposal.start_block);

        vote.voter = *ctx.accounts.voter.key;
        vote.proposal_id = proposal_id;
        vote.support = support;
        vote.weight = voter_weight;

        if support {
            proposal.for_votes += voter_weight;
        } else {
            proposal.against_votes += voter_weight;
        }

        emit!(VoteCast {
            voter: vote.voter,
            proposal_id,
            support,
            weight: voter_weight,
        });

        Ok(())
    }

    pub fn execute_proposal(ctx: Context<ExecuteProposal>, proposal_id: u64) -> Result<()> {
        let governor = &ctx.accounts.governor;
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;

        require!(!proposal.executed, GovernorError::ProposalAlreadyExecuted);
        require!(!proposal.canceled, GovernorError::ProposalCanceled);
        require!(clock.slot > proposal.end_block, GovernorError::VotingPeriodActive);

        let proposal_type_info = governor.proposal_types.get(&proposal.proposal_type).unwrap();
        let quorum = (governor.total_supply * proposal_type_info.quorum as u64) / 10_000;
        let approval_threshold = (proposal.for_votes * 10_000) / (proposal.for_votes + proposal.against_votes);

        require!(
            proposal.for_votes + proposal.against_votes >= quorum,
            GovernorError::QuorumNotReached
        );
        require!(
            approval_threshold >= proposal_type_info.approval_threshold as u64,
            GovernorError::ApprovalThresholdNotMet
        );

        // TODO: Execute proposal logic here
        // This would typically involve calling other instructions or programs

        proposal.executed = true;

        emit!(ProposalExecuted { proposal_id });

        Ok(())
    }

    // TODO: Add more instructions for other functionalities like canceling proposals, 
    // setting proposal types, updating governor settings, etc.
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = admin, space = 8 + Governor::LEN)]
    pub governor: Account<'info, Governor>,
    #[account(mut)]
    pub admin: Signer<'info>,
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub manager: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub governor: Account<'info, Governor>,
    #[account(init, payer = proposer, space = 8 + Proposal::LEN)]
    pub proposal: Account<'info, Proposal>,
    #[account(mut)]
    pub proposer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CastVote<'info> {
    #[account(mut)]
    pub governor: Account<'info, Governor>,
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    #[account(init, payer = voter, space = 8 + Vote::LEN)]
    pub vote: Account<'info, Vote>,
    #[account(mut)]
    pub voter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    #[account(mut)]
    pub governor: Account<'info, Governor>,
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
    pub executor: Signer<'info>,
}

#[account]
pub struct Governor {
    pub admin: Pubkey,
    pub manager: Pubkey,
    pub voting_delay: u64,
    pub voting_period: u64,
    pub proposal_threshold: u64,
    pub proposal_count: u64,
    pub total_supply: u64,
    pub proposal_types: Vec<ProposalType>,
}

#[account]
pub struct Proposal {
    pub id: u64,
    pub proposer: Pubkey,
    pub description: String,
    pub proposal_type: u8,
    pub start_block: u64,
    pub end_block: u64,
    pub for_votes: u64,
    pub against_votes: u64,
    pub executed: bool,
    pub canceled: bool,
}

#[account]
pub struct Vote {
    pub voter: Pubkey,
    pub proposal_id: u64,
    pub support: bool,
    pub weight: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct ProposalType {
    pub quorum: u16,
    pub approval_threshold: u16,
    pub name: String,
    pub module: Option<Pubkey>,
}

#[error_code]
pub enum GovernorError {
    #[msg("Proposer does not have enough votes to create a proposal")]
    InsufficientProposerVotes,
    #[msg("Invalid proposal type")]
    InvalidProposalType,
    #[msg("Voting period is not active")]
    VotingPeriodInactive,
    #[msg("Proposal has already been executed")]
    ProposalAlreadyExecuted,
    #[msg("Proposal has been canceled")]
    ProposalCanceled,
    #[msg("Voting period is still active")]
    VotingPeriodActive,
    #[msg("Quorum not reached")]
    QuorumNotReached,
    #[msg("Approval threshold not met")]
    ApprovalThresholdNotMet,
}

impl Governor {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 8 + 8 + 32;

    pub fn get_votes(&self, account: &Pubkey, block: u64) -> u64 {
        // TODO: Implement logic to get votes for an account at a specific block
        // This would typically involve querying a token account or stake account
        0
    }
}

impl Proposal {
    pub const LEN: usize = 8 + 32 + 200 + 1 + 8 + 8 + 8 + 8 + 1 + 1;
}

impl Vote {
    pub const LEN: usize = 32 + 8 + 1 + 8;
}

#[event]
pub struct ProposalCreated {
    pub proposal_id: u64,
    pub proposer: Pubkey,
    pub start_block: u64,
    pub end_block: u64,
    pub description: String,
    pub proposal_type: u8,
}

#[event]
pub struct VoteCast {
    pub voter: Pubkey,
    pub proposal_id: u64,
    pub support: bool,
    pub weight: u64,
}

#[event]
pub struct ProposalExecuted {
    pub proposal_id: u64,
}