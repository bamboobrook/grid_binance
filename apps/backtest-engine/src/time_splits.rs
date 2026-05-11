#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeWindow {
    pub name: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkForwardWindow {
    pub train: TimeWindow,
    pub validate: TimeWindow,
    pub test: TimeWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkForwardConfig {
    pub start_ms: i64,
    pub end_ms: i64,
    pub train_ms: i64,
    pub validate_ms: i64,
    pub test_ms: i64,
    pub step_ms: i64,
}

pub fn manual_windows(windows: Vec<TimeWindow>) -> Result<Vec<TimeWindow>, String> {
    validate_ordered_windows(&windows)?;
    Ok(windows)
}

pub fn walk_forward_windows(config: WalkForwardConfig) -> Result<Vec<WalkForwardWindow>, String> {
    validate_positive("train_ms", config.train_ms)?;
    validate_positive("validate_ms", config.validate_ms)?;
    validate_positive("test_ms", config.test_ms)?;
    validate_positive("step_ms", config.step_ms)?;
    if config.start_ms >= config.end_ms {
        return Err("start_ms must be before end_ms".to_string());
    }

    let train_validate_ms = config
        .train_ms
        .checked_add(config.validate_ms)
        .ok_or_else(|| "walk-forward span overflow".to_string())?;
    let span = train_validate_ms
        .checked_add(config.test_ms)
        .ok_or_else(|| "walk-forward span overflow".to_string())?;
    let mut start = config.start_ms;
    let mut windows = Vec::new();
    loop {
        let test_end_for_loop = start
            .checked_add(span)
            .ok_or_else(|| "walk-forward window overflow".to_string())?;
        if test_end_for_loop > config.end_ms {
            break;
        }

        let train_end = start
            .checked_add(config.train_ms)
            .ok_or_else(|| "walk-forward train window overflow".to_string())?;
        let validate_end = train_end
            .checked_add(config.validate_ms)
            .ok_or_else(|| "walk-forward validate window overflow".to_string())?;
        let test_end = validate_end
            .checked_add(config.test_ms)
            .ok_or_else(|| "walk-forward test window overflow".to_string())?;
        windows.push(WalkForwardWindow {
            train: TimeWindow {
                name: format!("wf-{}-train", windows.len() + 1),
                start_ms: start,
                end_ms: train_end,
            },
            validate: TimeWindow {
                name: format!("wf-{}-validate", windows.len() + 1),
                start_ms: train_end,
                end_ms: validate_end,
            },
            test: TimeWindow {
                name: format!("wf-{}-test", windows.len() + 1),
                start_ms: validate_end,
                end_ms: test_end,
            },
        });
        start = start
            .checked_add(config.step_ms)
            .ok_or_else(|| "walk-forward step overflow".to_string())?;
    }

    Ok(windows)
}

pub fn named_stress_windows() -> Vec<TimeWindow> {
    vec![
        TimeWindow {
            name: "crash".to_string(),
            start_ms: 1_583_020_800_000,
            end_ms: 1_585_785_600_000,
        },
        TimeWindow {
            name: "melt_up".to_string(),
            start_ms: 1_609_459_200_000,
            end_ms: 1_622_419_200_000,
        },
        TimeWindow {
            name: "high_volatility_chop".to_string(),
            start_ms: 1_667_692_800_000,
            end_ms: 1_670_284_800_000,
        },
        TimeWindow {
            name: "low_volatility_range".to_string(),
            start_ms: 1_688_169_600_000,
            end_ms: 1_696_032_000_000,
        },
        TimeWindow {
            name: "long_unidirectional_trend".to_string(),
            start_ms: 1_699_660_800_000,
            end_ms: 1_709_596_800_000,
        },
        TimeWindow {
            name: "wick_spike".to_string(),
            start_ms: 1_617_840_000_000,
            end_ms: 1_617_926_400_000,
        },
    ]
}

pub fn stress_window_by_name(name: &str) -> Option<TimeWindow> {
    named_stress_windows()
        .into_iter()
        .find(|window| window.name == name)
}

fn validate_ordered_windows(windows: &[TimeWindow]) -> Result<(), String> {
    let mut previous_end = None;
    for window in windows {
        if window.start_ms >= window.end_ms {
            return Err(format!("{} start_ms must be before end_ms", window.name));
        }
        if previous_end.is_some_and(|end| window.start_ms < end) {
            return Err("manual windows must be ordered and non-overlapping".to_string());
        }
        previous_end = Some(window.end_ms);
    }
    Ok(())
}

fn validate_positive(name: &str, value: i64) -> Result<(), String> {
    if value <= 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(())
}
