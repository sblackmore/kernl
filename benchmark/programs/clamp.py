def clamp(val: int, lo: int, hi: int) -> int:
    result = max(lo, min(hi, val))
    assert result >= lo
    assert result <= hi
    return result
