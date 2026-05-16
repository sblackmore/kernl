fn clamp(val: i64, lo: i64, hi: i64) -> i64 {
    let result = lo.max(hi.min(val));
    assert!(result >= lo);
    assert!(result <= hi);
    result
}
