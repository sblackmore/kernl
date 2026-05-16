fn sum_positive(nums: &[i64]) -> i64 {
    let result: i64 = nums.iter().filter(|&&x| x > 0).sum();
    assert!(result >= 0);
    result
}
