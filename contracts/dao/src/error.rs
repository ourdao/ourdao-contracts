use soroban_sdk::contracterror;

/// Every failure mode the DAO can return. Numeric codes are stable and part of
/// the contract's public ABI, so append new variants rather than renumbering.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    // ---- lifecycle / config ----
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidThreshold = 3,
    InvalidAmount = 4,
    InvalidLoanPolicy = 5,
    Paused = 6,
    NotPaused = 7,

    // ---- authorization ----
    NotAuthorized = 10,
    NotAdmin = 11,
    NotMember = 12,
    AlreadyAdmin = 13,
    AlreadyMember = 14,
    CannotRemoveLastAdmin = 15,

    // ---- membership ----
    MemberNotActive = 20,
    HasActiveLoan = 21,

    // ---- loans ----
    ProposalNotFound = 30,
    NotBorrower = 31,
    NotInEditingPhase = 32,
    NotInVotingPhase = 33,
    VotingEnded = 34,
    AlreadyVoted = 35,
    NotEligibleForLoan = 36,
    CooldownActive = 37,
    LoanNotFound = 38,
    LoanNotActive = 39,
    ExceedsTreasuryRatio = 40,
    InsufficientTreasury = 41,

    // ---- treasury ----
    TreasuryProposalNotFound = 50,

    // ---- native-swap modules ----
    NameTaken = 60,
    NameNotFound = 61,
    NoStake = 62,
    InsufficientStake = 63,
    NoCommitment = 64,
    CommitmentMismatch = 65,
    AlreadyRevealed = 66,
    NothingToClaim = 67,
}
