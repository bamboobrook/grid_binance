from optimize_margin_v2_lp_portfolios import per_strategy_weight_pct


def test_long_short_pair_splits_weight_equally():
    # A 36% long_short candidate has 2 internal strategies -> 18% each, pair sums to 36%.
    assert per_strategy_weight_pct(36.0, 2) == 18.0
    assert per_strategy_weight_pct(36.0, 1) == 36.0
    assert per_strategy_weight_pct(100.0, 2) == 50.0


def test_zero_internal_count_is_safe():
    assert per_strategy_weight_pct(40.0, 0) == 40.0


if __name__ == "__main__":
    test_long_short_pair_splits_weight_equally()
    test_zero_internal_count_is_safe()
    print("ok")
