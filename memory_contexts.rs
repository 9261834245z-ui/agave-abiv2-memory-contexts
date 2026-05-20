use {
    crate::invoke_context::BpfAllocator,
    solana_instruction::error::InstructionError,
    solana_sbpf::{
        ebpf::{MM_BYTECODE_START, MM_RODATA_START},
        elf::Executable,
        memory_region::{
            default_access_violation_handler,
            MemoryMapping,
            MemoryRegion,
        },
        program::SBPFVersion,
        vm::{Config, ContextObject},
    },
    solana_transaction_context::{
        transaction::TransactionContext,
        vm_addresses::{
            abiv2_region_index_from_vm_address,
            ACCOUNT_METADATA_AREA,
            GUEST_ACCOUNT_PAYLOAD_BASE_ADDRESS,
            GUEST_INSTRUCTION_ACCOUNT_BASE_ADDRESS,
            GUEST_INSTRUCTION_DATA_BASE_ADDRESS,
            HEAP_ADDRESS,
            INSTRUCTION_TRACE_AREA,
            RETURN_DATA_SCRATCHPAD,
            STACK_ADDRESS,
            TRANSACTION_FRAME_ADDRESS,
        },
        MAX_ACCOUNTS_PER_TRANSACTION,
        MAX_INSTRUCTION_TRACE_LENGTH,
    },
};

/// Dynamically derived region count.
///
/// Prevents silent layout drift if upstream VM address layout changes.
const NUMBER_OF_REGIONS: usize =
    abiv2_region_index_from_vm_address(
        GUEST_INSTRUCTION_ACCOUNT_BASE_ADDRESS,
    ) + MAX_INSTRUCTION_TRACE_LENGTH;

/// Cached region boundaries.
///
/// Avoid repeated recomputation and ensure consistency.
const ACCOUNT_REGION_START: usize =
    abiv2_region_index_from_vm_address(
        GUEST_ACCOUNT_PAYLOAD_BASE_ADDRESS,
    );

const ACCOUNT_REGION_END: usize =
    ACCOUNT_REGION_START + MAX_ACCOUNTS_PER_TRANSACTION;

const INSTRUCTION_DATA_REGION_START: usize =
    abiv2_region_index_from_vm_address(
        GUEST_INSTRUCTION_DATA_BASE_ADDRESS,
    );

const INSTRUCTION_DATA_REGION_END: usize =
    INSTRUCTION_DATA_REGION_START
        + MAX_INSTRUCTION_TRACE_LENGTH;

const INSTRUCTION_ACCOUNT_REGION_START: usize =
    abiv2_region_index_from_vm_address(
        GUEST_INSTRUCTION_ACCOUNT_BASE_ADDRESS,
    );

const INSTRUCTION_ACCOUNT_REGION_END: usize =
    INSTRUCTION_ACCOUNT_REGION_START
        + MAX_INSTRUCTION_TRACE_LENGTH;

enum MemoryContextType {
    ABIv1(MemoryContext),
    Placeholder,
    ABIv2,
}

/// Per-frame writable rollback storage.
///
/// This prevents writable permission leakage
/// across nested CPI execution frames.
#[derive(Default)]
struct FrameWritableSnapshot {
    entries: Vec<WritableSnapshot>,
}

#[derive(Clone, Copy)]
struct WritableSnapshot {
    region_index: usize,
    previous_writable: bool,
}

pub struct MemoryContexts {
    contexts: Vec<MemoryContextType>,

    abiv2_mappings: Box<MemoryMapping>,

    /// Writable rollback stack aligned with CPI depth.
    writable_snapshot_stack: Vec<FrameWritableSnapshot>,
}

impl MemoryContexts {
    pub(crate) fn new() -> Self {
        Self {
            contexts: Vec::new(),

            writable_snapshot_stack: Vec::new(),

            // Keep initialization ABI-consistent with ABIv2 path.
            abiv2_mappings: Box::new(unsafe {
                MemoryMapping::new_uninitialized(
                    Vec::new(),
                    &Config::default(),
                    SBPFVersion::Reserved,
                    Box::new(default_access_violation_handler),
                )
            }),
        }
    }

    pub fn set_memory_context_abi_v1(
        &mut self,
        memory_context: MemoryContext,
    ) -> Result<(), InstructionError> {
        *self
            .contexts
            .last_mut()
            .ok_or(InstructionError::CallDepth)? =
            MemoryContextType::ABIv1(memory_context);

        Ok(())
    }

    pub fn memory_context_abi_v1(
        &self,
    ) -> Result<&MemoryContext, InstructionError> {
        match self
            .contexts
            .last()
            .ok_or(InstructionError::CallDepth)?
        {
            MemoryContextType::ABIv1(ctx) => Ok(ctx),

            MemoryContextType::Placeholder => {
                Err(InstructionError::ProgramEnvironmentSetupFailure)
            }

            MemoryContextType::ABIv2 => {
                Err(InstructionError::InvalidAccountData)
            }
        }
    }

    pub fn memory_context_mut_abi_v1(
        &mut self,
    ) -> Result<&mut MemoryContext, InstructionError> {
        match self
            .contexts
            .last_mut()
            .ok_or(InstructionError::CallDepth)?
        {
            MemoryContextType::ABIv1(ctx) => Ok(ctx),

            MemoryContextType::Placeholder => {
                Err(InstructionError::ProgramEnvironmentSetupFailure)
            }

            MemoryContextType::ABIv2 => {
                Err(InstructionError::ProgramEnvironmentSetupFailure)
            }
        }
    }

    pub fn memory_mapping(
        &self,
    ) -> Result<&MemoryMapping, InstructionError> {
        match self
            .contexts
            .last()
            .ok_or(InstructionError::CallDepth)?
        {
            MemoryContextType::ABIv1(ctx) => {
                Ok(&ctx.memory_mapping)
            }

            MemoryContextType::Placeholder => {
                Err(InstructionError::ProgramEnvironmentSetupFailure)
            }

            MemoryContextType::ABIv2 => {
                Ok(&self.abiv2_mappings)
            }
        }
    }

    pub fn memory_mapping_mut(
        &mut self,
    ) -> Result<&mut MemoryMapping, InstructionError> {
        match self
            .contexts
            .last_mut()
            .ok_or(InstructionError::CallDepth)?
        {
            MemoryContextType::ABIv1(ctx) => {
                Ok(&mut ctx.memory_mapping)
            }

            MemoryContextType::Placeholder => {
                Err(InstructionError::ProgramEnvironmentSetupFailure)
            }

            MemoryContextType::ABIv2 => {
                Ok(&mut self.abiv2_mappings)
            }
        }
    }

    #[cfg(feature = "dev-context-only-utils")]
    pub fn mock_set_mapping_abi_v1(
        &mut self,
        memory_mapping: MemoryMapping,
    ) {
        self.contexts = vec![
            MemoryContextType::ABIv1(
                MemoryContext {
                    allocator: BpfAllocator::new(0),
                    accounts_metadata: vec![],
                    memory_mapping: Box::new(memory_mapping),
                },
            ),
        ];
    }

    /// Push new CPI frame placeholder.
    ///
    /// Snapshot stack remains depth-aligned.
    pub fn push_placeholder(&mut self) {
        self.contexts
            .push(MemoryContextType::Placeholder);

        self.writable_snapshot_stack
            .push(FrameWritableSnapshot::default());
    }

    /// Pop CPI frame and rollback writable mutations.
    pub fn pop(&mut self) {
        self.rollback_account_permissions();
        self.contexts.pop();
    }

    pub fn abi_v2_regions_exist(&self) -> bool {
        !self
            .abiv2_mappings
            .get_regions()
            .is_empty()
    }

    /// Create ABIv2 mappings.
    ///
    /// Unsafe initialization isolated here.
    pub fn create_abi_v2_mappings<C: ContextObject>(
        &mut self,
        regions: Vec<MemoryRegion>,
        executable: &Executable<C>,
    ) {
        *self.abiv2_mappings = unsafe {
            MemoryMapping::new_uninitialized(
                regions,
                executable.get_config(),
                executable.get_sbpf_version(),
                Box::new(default_access_violation_handler),
            )
        };
    }

    pub fn set_abi_v2(
        &mut self,
    ) -> Result<(), InstructionError> {
        *self
            .contexts
            .last_mut()
            .ok_or(InstructionError::CallDepth)? =
            MemoryContextType::ABIv2;

        Ok(())
    }

    /// Updates writable permissions for current instruction.
    ///
    /// Hardened against:
    /// - panic-based validator aborts
    /// - stale writable leakage
    /// - malformed account indexes
    /// - nested CPI corruption
    pub fn update_abi_v2_account_permissions(
        &mut self,
        transaction_context: &TransactionContext,
    ) -> Result<(), InstructionError> {
        let current_instruction =
            transaction_context
                .get_current_instruction_context()?;

        let snapshot_frame =
            self.writable_snapshot_stack
                .last_mut()
                .ok_or(
                    InstructionError::CallDepth,
                )?;

        snapshot_frame.entries.clear();

        let account_regions = self
            .abiv2_mappings
            .get_regions_mut()
            .get_mut(
                ACCOUNT_REGION_START
                    ..ACCOUNT_REGION_END,
            )
            .ok_or(
                InstructionError::ProgramEnvironmentSetupFailure,
            )?;

        for account in
            current_instruction.instruction_accounts()
        {
            let region_index =
                account.index_in_transaction as usize;

            let region =
                account_regions
                    .get_mut(region_index)
                    .ok_or(
                        InstructionError::NotEnoughAccountKeys,
                    )?;

            snapshot_frame.entries.push(
                WritableSnapshot {
                    region_index,
                    previous_writable:
                        region.writable,
                },
            );

            region.writable =
                account.is_writable();
        }

        Ok(())
    }

    /// Rollback writable permissions for current CPI frame.
    fn rollback_account_permissions(&mut self) {
        let Some(snapshot_frame) =
            self.writable_snapshot_stack.pop()
        else {
            return;
        };

        if snapshot_frame.entries.is_empty() {
            return;
        }

        let Some(account_regions) = self
            .abiv2_mappings
            .get_regions_mut()
            .get_mut(
                ACCOUNT_REGION_START
                    ..ACCOUNT_REGION_END,
            )
        else {
            return;
        };

        for snapshot in snapshot_frame.entries {
            if let Some(region) =
                account_regions.get_mut(snapshot.region_index)
            {
                region.writable =
                    snapshot.previous_writable;
            }
        }
    }
}

/// Per-instruction memory state.
pub struct MemoryContext {
    pub allocator: BpfAllocator,

    pub accounts_metadata:
        Vec<SerializedAccountMetadata>,

    memory_mapping: Box<MemoryMapping>,
}

impl MemoryContext {
    pub fn new(
        allocator: BpfAllocator,
        accounts_metadata: Vec<SerializedAccountMetadata>,
        memory_mapping: MemoryMapping,
    ) -> Self {
        Self {
            allocator,
            accounts_metadata,
            memory_mapping: Box::new(memory_mapping),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerializedAccountMetadata {
    /// Address of serialized account record.
    pub vm_addr: u64,

    pub original_data_len: usize,

    pub vm_data_addr: u64,

    pub vm_key_addr: u64,

    pub vm_lamports_addr: u64,

    pub vm_owner_addr: u64,
}

/// Creates ABIv2 memory regions.
///
/// Hardened against:
/// - region overflow
/// - upstream constant drift
/// - panic-based failures
/// - invalid VM layouts
pub(crate) fn create_abiv2_regions(
    transaction_context: &TransactionContext,
) -> Result<Vec<MemoryRegion>, InstructionError> {
    let mut regions =
        vec![MemoryRegion::default(); NUMBER_OF_REGIONS];

    let mut assign_region =
        |vm_addr: u64,
         region: MemoryRegion|
         -> Result<(), InstructionError> {
            let index =
                abiv2_region_index_from_vm_address(vm_addr);

            let slot =
                regions
                    .get_mut(index)
                    .ok_or(
                        InstructionError::ProgramEnvironmentSetupFailure,
                    )?;

            *slot = region;

            Ok(())
        };

    // Static VM regions
    for vm_addr in [
        MM_RODATA_START,
        MM_BYTECODE_START,
        HEAP_ADDRESS,
        STACK_ADDRESS,
    ] {
        let index =
            abiv2_region_index_from_vm_address(vm_addr);

        let region =
            regions
                .get_mut(index)
                .ok_or(
                    InstructionError::ProgramEnvironmentSetupFailure,
                )?;

        region.vm_addr = vm_addr;
    }

    // Transaction frame
    assign_region(
        TRANSACTION_FRAME_ADDRESS,
        MemoryRegion::new(
            transaction_context.transaction_frame_address(),
            TRANSACTION_FRAME_ADDRESS,
        ),
    )?;

    // Accounts metadata
    assign_region(
        ACCOUNT_METADATA_AREA,
        MemoryRegion::new(
            transaction_context
                .accounts()
                .shared_fields_as_raw_slice(),
            ACCOUNT_METADATA_AREA,
        ),
    )?;

    // Instruction trace
    assign_region(
        INSTRUCTION_TRACE_AREA,
        MemoryRegion::new(
            transaction_context
                .instruction_trace_as_raw_slice(),
            INSTRUCTION_TRACE_AREA,
        ),
    )?;

    // Return data
    assign_region(
        RETURN_DATA_SCRATCHPAD,
        MemoryRegion::new(
            transaction_context
                .return_data_as_raw_slice(),
            RETURN_DATA_SCRATCHPAD,
        ),
    )?;

    // Transaction account payload regions
    {
        let payload_regions =
            regions
                .get_mut(
                    ACCOUNT_REGION_START
                        ..ACCOUNT_REGION_END,
                )
                .ok_or(
                    InstructionError::ProgramEnvironmentSetupFailure,
                )?;

        transaction_context
            .accounts()
            .account_payload_regions(
                payload_regions,
            );
    }

    // Instruction payload regions
    {
        let instruction_regions =
            regions
                .get_mut(
                    INSTRUCTION_DATA_REGION_START
                        ..INSTRUCTION_DATA_REGION_END,
                )
                .ok_or(
                    InstructionError::ProgramEnvironmentSetupFailure,
                )?;

        transaction_context
            .instruction_payload_regions(
                instruction_regions,
            );
    }

    // Instruction account regions
    {
        let account_regions =
            regions
                .get_mut(
                    INSTRUCTION_ACCOUNT_REGION_START
                        ..INSTRUCTION_ACCOUNT_REGION_END,
                )
                .ok_or(
                    InstructionError::ProgramEnvironmentSetupFailure,
                )?;

        transaction_context
            .instruction_accounts_regions(
                account_regions,
            );
    }

    Ok(regions)
}