//! Shared-memory IPC protocol research module.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SharableTransactionRegion {
    pub offset: usize,
    pub length: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SharablePubkeys {
    pub offset: usize,
    pub num_pubkeys: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct SharableTransactionBatchRegion {
    pub num_transactions: u8,
    pub transactions_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TransactionResponseRegion {
    pub tag: u8,
    pub num_transaction_responses: u8,
    pub transaction_responses_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct TpuToPackMessage {
    pub transaction: SharableTransactionRegion,
    pub flags: u8,
    pub src_addr: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProgressMessage {
    pub leader_state: u8,
    pub current_slot_progress: u8,
    pub epoch: u64,
    pub current_slot: u64,
    pub next_leader_slot: u64,
    pub leader_range_end: u64,
    pub remaining_cost_units: u64,
    pub latest_blockhash: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PackToWorkerMessage {
    pub flags: u16,
    pub max_working_slot: u64,
    pub batch: SharableTransactionBatchRegion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct WorkerToPackMessage {
    pub batch: SharableTransactionBatchRegion,
    pub processed_code: u8,
    pub responses: TransactionResponseRegion,
}

pub mod tpu_message_flags {
    pub const NONE: u8 = 0;
    pub const IS_SIMPLE_VOTE: u8 = 1 << 0;
    pub const FORWARDED: u8 = 1 << 1;
    pub const FROM_STAKED_NODE: u8 = 1 << 2;
}

pub mod pack_message_flags {
    pub const CHECK: u16 = 0;
    pub const EXECUTE: u16 = 1;

    pub mod execution_flags {
        pub const DROP_ON_FAILURE: u16 = 1 << 1;
        pub const ALL_OR_NOTHING: u16 = 1 << 2;
    }

    pub mod check_flags {
        pub const STATUS_CHECKS: u16 = 1 << 1;
        pub const LOAD_FEE_PAYER_BALANCE: u16 = 1 << 2;
        pub const LOAD_ADDRESS_LOOKUP_TABLES: u16 = 1 << 3;
    }
}

pub mod processed_codes {
    pub const PROCESSED: u8 = 0;
    pub const INVALID: u8 = 1;
    pub const MAX_WORKING_SLOT_EXCEEDED: u8 = 2;
}

pub mod worker_message_types {
    use super::SharablePubkeys;

    pub const EXECUTION_RESPONSE: u8 = 0;
    pub const CHECK_RESPONSE: u8 = 1;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(C)]
    pub struct ExecutionResponse {
        pub execution_slot: u64,
        pub not_included_reason: u8,
        pub cost_units: u64,
        pub fee_payer_balance: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(C)]
    pub struct CheckResponse {
        pub parsing_and_sanitization_flags: u8,
        pub status_check_flags: u8,
        pub fee_payer_balance_flags: u8,
        pub resolve_flags: u8,
        pub included_slot: u64,
        pub balance_slot: u64,
        pub fee_payer_balance: u64,
        pub resolution_slot: u64,
        pub min_alt_deactivation_slot: u64,
        pub resolved_pubkeys: SharablePubkeys,
    }
}
