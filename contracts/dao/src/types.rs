use soroban_sdk::{contracttype, Address, String};

/// Basis-points denominator (100% == 10_000). Mirrors the original EVM contract.
pub const BASIS_POINTS: i128 = 10_000;
/// Treasury withdrawals use a fixed, higher bar: 60%.
pub const TREASURY_THRESHOLD: u32 = 6_000;
/// A freshly filed loan proposal stays editable for this long before voting opens.
pub const PROPOSAL_EDITING_PERIOD: u64 = 3 * 24 * 60 * 60; // 3 days
/// Voting window length once a proposal enters the voting phase.
pub const VOTING_PERIOD: u64 = 7 * 24 * 60 * 60; // 7 days

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberStatus {
    ActiveMember,
    Inactive,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Executed,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalPhase {
    Editing,
    Voting,
    Executed,
    Expired,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoanStatus {
    Active,
    Repaid,
    Defaulted,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub address: Address,
    pub status: MemberStatus,
    pub join_ledger: u64,
    pub contribution: i128,
    pub share_balance: i128,
    pub has_active_loan: bool,
    pub last_loan_time: u64,
}

/// Tunable lending parameters. Durations are in ledger seconds; rates in bps.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanPolicy {
    pub min_membership_duration: u64,
    pub membership_contribution: i128,
    pub max_loan_duration: u64,
    pub min_interest_rate: u32,
    pub max_interest_rate: u32,
    pub cooldown_period: u64,
    pub max_loan_to_treasury_ratio: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanProposal {
    pub id: u32,
    pub borrower: Address,
    pub amount: i128,
    pub interest_rate: u32,
    pub duration: u64,
    pub total_repayment: i128,
    pub created_at: u64,
    pub editing_period_end: u64,
    pub phase: ProposalPhase,
    pub status: ProposalStatus,
    pub for_votes: i128,
    pub against_votes: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Loan {
    pub id: u32,
    pub borrower: Address,
    pub principal: i128,
    pub interest_rate: u32,
    pub total_repayment: i128,
    pub start_time: u64,
    pub due_time: u64,
    pub status: LoanStatus,
    pub amount_repaid: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreasuryProposal {
    pub id: u32,
    pub proposer: Address,
    pub amount: i128,
    pub destination: Address,
    pub reason: String,
    pub created_at: u64,
    pub status: ProposalStatus,
    pub for_votes: i128,
    pub against_votes: i128,
    /// When true, votes must be committed then revealed (commit-reveal privacy).
    pub private: bool,
}

/// Computed loan terms returned by the read-only quote helper.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoanTerms {
    pub interest_rate: u32,
    pub total_repayment: i128,
    pub duration: u64,
}
