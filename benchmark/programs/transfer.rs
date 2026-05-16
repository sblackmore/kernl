struct Account {
    id: u64,
    balance: i64,
    owner: String,
}

fn transfer(amount: u64, from: &mut Account, to: &mut Account) {
    assert!(from.balance >= amount as i64);
    from.balance -= amount as i64;
    to.balance += amount as i64;
}
