//! Conflict-aware scheduling research module.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConflictScore {
    pub hot_account_conflicts: u32,
    pub dependency_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTransaction {
    pub id: u64,
    pub read_accounts: Vec<u64>,
    pub write_accounts: Vec<u64>,
}

#[derive(Debug, Default)]
pub struct Scheduler {
    pub transactions: Vec<ScheduledTransaction>,
}

impl Scheduler {
    pub fn score_conflicts(&self, tx: &ScheduledTransaction) -> ConflictScore {
        let hot_account_conflicts = tx
            .read_accounts
            .iter()
            .chain(tx.write_accounts.iter())
            .filter(|account| {
                let account = **account;
                account % 7 == 0 || account % 7 == 1
            })
            .count() as u32;

        let dependency_depth = tx
            .write_accounts
            .len()
            .max(tx.read_accounts.len()) as u32;

        ConflictScore {
            hot_account_conflicts,
            dependency_depth,
        }
    }

    pub fn sort_by_conflict(&mut self) {
        let scores: Vec<(u64, ConflictScore)> = self
            .transactions
            .iter()
            .map(|tx| (tx.id, self.score_conflicts(tx)))
            .collect();

        self.transactions.sort_by_key(|tx| {
            let score = scores
                .iter()
                .find(|(id, _)| *id == tx.id)
                .map(|(_, score)| *score)
                .unwrap_or_default();
            (score.hot_account_conflicts, score.dependency_depth, tx.id)
        });
    }
}
