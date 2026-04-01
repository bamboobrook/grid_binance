use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use crate::runtime::GridMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridPlan {
    pub mode: GridMode,
    pub levels: Vec<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridBuildError {
    message: String,
}

impl GridBuildError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for GridBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GridBuildError {}

pub struct GridBuilder;

impl GridBuilder {
    pub fn arithmetic(
        mode: GridMode,
        lower: Decimal,
        upper: Decimal,
        levels: usize,
    ) -> Result<GridPlan, GridBuildError> {
        validate_range(lower, upper, levels)?;
        let step = (upper - lower) / Decimal::from((levels - 1) as u64);
        let built_levels = (0..levels)
            .map(|index| lower + step * Decimal::from(index as u64))
            .collect();

        Ok(GridPlan {
            mode,
            levels: built_levels,
        })
    }

    pub fn geometric(
        mode: GridMode,
        lower: Decimal,
        upper: Decimal,
        levels: usize,
    ) -> Result<GridPlan, GridBuildError> {
        validate_range(lower, upper, levels)?;
        if lower <= Decimal::ZERO {
            return Err(GridBuildError::new("lower bound must be positive"));
        }

        let lower_f64 = lower
            .to_f64()
            .ok_or_else(|| GridBuildError::new("failed to convert lower bound"))?;
        let upper_f64 = upper
            .to_f64()
            .ok_or_else(|| GridBuildError::new("failed to convert upper bound"))?;
        let ratio = (upper_f64 / lower_f64).powf(1.0 / (levels - 1) as f64);

        let mut built_levels = Vec::with_capacity(levels);
        for index in 0..levels {
            if index == 0 {
                built_levels.push(lower);
                continue;
            }

            if index == levels - 1 {
                built_levels.push(upper);
                continue;
            }

            let level = lower_f64 * ratio.powi(index as i32);
            let level = Decimal::from_f64(level)
                .ok_or_else(|| GridBuildError::new("failed to convert geometric level"))?
                .round_dp(8)
                .normalize();
            built_levels.push(level);
        }

        if built_levels.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GridBuildError::new(
                "geometric grid levels must remain strictly increasing after rounding",
            ));
        }

        Ok(GridPlan {
            mode,
            levels: built_levels,
        })
    }

    pub fn custom(mode: GridMode, levels: Vec<Decimal>) -> Result<GridPlan, GridBuildError> {
        if levels.is_empty() {
            return Err(GridBuildError::new("grid requires at least one level"));
        }

        if levels.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GridBuildError::new(
                "custom grid levels must be strictly increasing",
            ));
        }

        Ok(GridPlan { mode, levels })
    }
}

fn validate_range(lower: Decimal, upper: Decimal, levels: usize) -> Result<(), GridBuildError> {
    if levels < 2 {
        return Err(GridBuildError::new("grid requires at least two levels"));
    }

    if lower >= upper {
        return Err(GridBuildError::new(
            "lower bound must be smaller than upper bound",
        ));
    }

    Ok(())
}
