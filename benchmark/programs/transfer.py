from dataclasses import dataclass

@dataclass
class Account:
    id: int
    balance: int
    owner: str

def transfer(amount: int, from_acct: Account, to_acct: Account) -> None:
    assert from_acct.balance >= amount
    from_acct.balance -= amount
    to_acct.balance += amount
