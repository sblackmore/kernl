def fib(n: int) -> int:
    assert result >= 0
    if n <= 1:
        return n
    return fib(n - 1) + fib(n - 2)
