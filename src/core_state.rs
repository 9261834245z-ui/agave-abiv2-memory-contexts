#[derive(Debug, Clone, Default)]
pub struct CoreState {
    pub p: u64,
    pub global_field: i128,
    pub total_supply: u128,
    pub total_minted: u128,
    pub total_burned: u128,
    pub total_base_sum: i128,
}
impl CoreState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn check_invariant(&self) -> Result<(), &'static str> {
        if self.total_supply as i128 != self.total_minted as i128 - self.total_burned as i128 {
            return Err("supply invariant");
        }
        if self.total_supply as i128
            != self.total_base_sum + self.global_field.checked_mul(self.p as i128).unwrap_or(0)
        {
            return Err("base sum invariant");
        }
        Ok(())
    }
    pub fn register_participant(&mut self) -> Result<(), &'static str> {
        self.total_minted = self.total_minted.saturating_add(1);
        self.total_supply = self.total_minted.saturating_sub(self.total_burned);
        self.total_base_sum =
            self.total_supply as i128 - self.global_field.checked_mul(self.p as i128).unwrap_or(0);
        Ok(())
    }
    pub fn unregister_participant(&mut self, base_balance: i128) -> Result<(), &'static str> {
        let (mint_delta, burn_delta): (u128, u128) = if base_balance >= 0 {
            let extra = base_balance as u128;
            (0, 1u128.saturating_add(extra))
        } else {
            (base_balance.unsigned_abs(), 1)
        };

        let new_minted = self
            .total_minted
            .checked_add(mint_delta)
            .ok_or("mint overflow")?;
        let new_burned = self
            .total_burned
            .checked_add(burn_delta)
            .ok_or("burn overflow")?;
        if new_burned > new_minted {
            return Err("supply invariant");
        }

        self.total_minted = new_minted;
        self.total_burned = new_burned;
        self.total_supply = new_minted - new_burned;
        self.total_base_sum =
            self.total_supply as i128 - self.global_field.checked_mul(self.p as i128).unwrap_or(0);
        Ok(())
    }
    pub fn apply_transfer(
        &mut self,
        _from: u64,
        _to: u64,
        amount: u128,
        edge_cost: i128,
    ) -> Result<(), &'static str> {
        let (mint_delta, burn_delta): (u128, u128) = if edge_cost >= 0 {
            let extra = edge_cost as u128;
            (amount.checked_add(extra).ok_or("mint overflow")?, 0)
        } else {
            (amount, edge_cost.unsigned_abs())
        };

        let new_minted = self
            .total_minted
            .checked_add(mint_delta)
            .ok_or("mint overflow")?;
        let new_burned = self
            .total_burned
            .checked_add(burn_delta)
            .ok_or("burn overflow")?;
        if new_burned > new_minted {
            return Err("supply invariant");
        }

        self.total_minted = new_minted;
        self.total_burned = new_burned;
        self.total_supply = new_minted - new_burned;
        self.total_base_sum =
            self.total_supply as i128 - self.global_field.checked_mul(self.p as i128).unwrap_or(0);
        Ok(())
    }
    pub fn redistribute_amount(&mut self, amount: u128) -> Result<(), &'static str> {
        self.total_supply = self.total_supply.saturating_add(amount);
        self.total_minted = self.total_minted.saturating_add(amount);
        self.total_base_sum =
            self.total_supply as i128 - self.global_field.checked_mul(self.p as i128).unwrap_or(0);
        Ok(())
    }
    pub fn apply_neg_entropy_tick(&mut self) -> Result<(), &'static str> {
        self.global_field = self.global_field.saturating_add(1);
        self.total_base_sum =
            self.total_supply as i128 - self.global_field.checked_mul(self.p as i128).unwrap_or(0);
        Ok(())
    }
}
