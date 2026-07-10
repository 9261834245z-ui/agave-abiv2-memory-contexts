use agave_abiv2_memory_contexts::memory_contexts::{MemoryContexts, PermissionState};
use agave_abiv2_memory_contexts::scheduler::{ConflictScore, Scheduler, ScheduledTransaction};
use agave_abiv2_memory_contexts::shared_memory_protocol::{
    pack_message_flags, processed_codes, PackToWorkerMessage, ProgressMessage,
    SharableTransactionBatchRegion, TpuToPackMessage, WorkerToPackMessage,
};

#[test]
fn nested_cpi_writable_permissions_are_restored() {
    let mut contexts = MemoryContexts::default();
    contexts.push_placeholder();
    contexts.update_account_permissions(&[(0, true), (2, true)]);
    assert_eq!(contexts.region(0), Some(PermissionState::Writable));
    assert_eq!(contexts.region(2), Some(PermissionState::Writable));

    contexts.push_placeholder();
    contexts.update_account_permissions(&[(0, false)]);
    assert_eq!(contexts.region(0), Some(PermissionState::ReadOnly));

    contexts.pop();
    assert_eq!(contexts.region(0), Some(PermissionState::Writable));

    contexts.pop();
    assert_eq!(contexts.region(0), Some(PermissionState::Writable));
}

#[test]
fn shared_memory_layout_message_roundtrip_is_stable() {
    let batch = SharableTransactionBatchRegion {
        num_transactions: 3,
        transactions_offset: 0x1000,
    };

    let request = PackToWorkerMessage {
        flags: pack_message_flags::EXECUTE | pack_message_flags::execution_flags::ALL_OR_NOTHING,
        max_working_slot: 42,
        batch,
    };

    let response = WorkerToPackMessage {
        batch,
        processed_code: processed_codes::PROCESSED,
        responses: agave_abiv2_memory_contexts::shared_memory_protocol::TransactionResponseRegion {
            tag: 0,
            num_transaction_responses: 3,
            transaction_responses_offset: 0x2000,
        },
    };

    let tpu_message = TpuToPackMessage {
        transaction: agave_abiv2_memory_contexts::shared_memory_protocol::SharableTransactionRegion {
            offset: 0x3000,
            length: 64,
        },
        flags: agave_abiv2_memory_contexts::shared_memory_protocol::tpu_message_flags::FORWARDED,
        src_addr: [0x01; 16],
    };

    let progress = ProgressMessage {
        leader_state: 2,
        current_slot_progress: 30,
        epoch: 7,
        current_slot: 42,
        next_leader_slot: 43,
        leader_range_end: 45,
        remaining_cost_units: 1000,
        latest_blockhash: [0xAA; 32],
    };

    assert_eq!(request.batch.num_transactions, 3);
    assert_eq!(response.batch.num_transactions, 3);
    assert_eq!(tpu_message.flags, agave_abiv2_memory_contexts::shared_memory_protocol::tpu_message_flags::FORWARDED);
    assert_eq!(progress.leader_state, 2);
}

#[test]
fn conflict_scoring_prefers_hot_accounts_and_depth() {
    let mut scheduler = Scheduler::default();
    scheduler.transactions.push(ScheduledTransaction {
        id: 2,
        read_accounts: vec![1, 2, 3],
        write_accounts: vec![7],
    });
    scheduler.transactions.push(ScheduledTransaction {
        id: 1,
        read_accounts: vec![7, 8],
        write_accounts: vec![9],
    });

    scheduler.sort_by_conflict();

    let first = scheduler.transactions.first().unwrap();
    assert_eq!(first.id, 1);
    let score = scheduler.score_conflicts(first);
    assert_eq!(score, ConflictScore { hot_account_conflicts: 2, dependency_depth: 2 });
}
