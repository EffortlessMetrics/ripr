pub struct Ledger {
    balance: i32,
}

impl Ledger {
    pub fn new(balance: i32) -> Self {
        Self { balance }
    }

    pub fn apply(&mut self, amount: i32) {
        self.persist(amount * 9).ok();
    }

    fn persist(&mut self, delta: i32) -> Result<(), String> {
        self.balance += delta;
        Ok(())
    }

    pub fn balance(&self) -> i32 {
        self.balance
    }
}
