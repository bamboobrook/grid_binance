use rust_decimal::Decimal;

use crate::runtime::GridMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridPlan {
    pub mode: GridMode,
    pub levels: Vec<Decimal>,
    pub lower_levels: Vec<Decimal>,
    pub upper_levels: Vec<Decimal>,
}

impl GridPlan {
    pub fn validate_shape(&self) -> Result<(), GridBuildError> {
        match self.mode {
            GridMode::SpotGrid | GridMode::FuturesLong | GridMode::FuturesShort => {
                if self.levels.is_empty() {
                    return Err(GridBuildError::new(
                        "ordinary grid plan requires at least one level",
                    ));
                }

                if !self.lower_levels.is_empty() || !self.upper_levels.is_empty() {
                    return Err(GridBuildError::new(
                        "ordinary grid plan cannot include bilateral levels",
                    ));
                }

                Ok(())
            }
            GridMode::ClassicBilateralSpot | GridMode::ClassicBilateralFutures => {
                if !self.levels.is_empty() {
                    return Err(GridBuildError::new(
                        "classic bilateral grid plan cannot include ordinary levels",
                    ));
                }

                if self.lower_levels.is_empty() || self.upper_levels.is_empty() {
                    return Err(GridBuildError::new(
                        "classic bilateral grid plan requires lower and upper levels",
                    ));
                }

                if self.lower_levels.len() != self.upper_levels.len() {
                    return Err(GridBuildError::new(
                        "classic bilateral grid plan requires symmetric side counts",
                    ));
                }

                validate_bilateral_levels(&self.lower_levels, &self.upper_levels)
            }
        }
    }

    fn ordinary(mode: GridMode, levels: Vec<Decimal>) -> Self {
        Self {
            mode,
            levels,
            lower_levels: Vec::new(),
            upper_levels: Vec::new(),
        }
    }

    fn bilateral(mode: GridMode, lower_levels: Vec<Decimal>, upper_levels: Vec<Decimal>) -> Self {
        Self {
            mode,
            levels: Vec::new(),
            lower_levels,
            upper_levels,
        }
    }
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
    pub fn ordinary_fixed_step(
        mode: GridMode,
        anchor_price: Decimal,
        spacing_bps: u32,
        grid_count: u32,
    ) -> Result<GridPlan, GridBuildError> {
        validate_ordinary_mode(mode)?;
        validate_anchor_inputs(anchor_price, spacing_bps, grid_count)?;

        let spacing = spacing_ratio(spacing_bps);
        let step = normalize_level(anchor_price * spacing);
        let mut levels = Vec::with_capacity(grid_count as usize);

        for index in 0..grid_count {
            let offset = step * Decimal::from(index);
            let level = match mode {
                GridMode::SpotGrid | GridMode::FuturesLong => anchor_price - offset,
                GridMode::FuturesShort => anchor_price + offset,
                GridMode::ClassicBilateralSpot | GridMode::ClassicBilateralFutures => {
                    unreachable!("ordinary mode validated above")
                }
            };
            let level = normalize_level(level);
            if level <= Decimal::ZERO {
                return Err(GridBuildError::new(
                    "ordinary grid levels must stay positive",
                ));
            }
            levels.push(level);
        }

        validate_ordinary_direction(mode, &levels)?;

        let plan = GridPlan::ordinary(mode, levels);
        plan.validate_shape()?;
        Ok(plan)
    }

    pub fn classic_bilateral_fixed(
        mode: GridMode,
        center_price: Decimal,
        spacing_bps: u32,
        levels_per_side: u32,
    ) -> Result<GridPlan, GridBuildError> {
        validate_bilateral_mode(mode)?;
        validate_anchor_inputs(center_price, spacing_bps, levels_per_side)?;

        let spacing = spacing_ratio(spacing_bps);
        let step = normalize_level(center_price * spacing);
        let mut lower_levels = Vec::with_capacity(levels_per_side as usize);
        let mut upper_levels = Vec::with_capacity(levels_per_side as usize);

        for index in 1..=levels_per_side {
            let offset = step * Decimal::from(index);
            let lower = normalize_level(center_price - offset);
            let upper = normalize_level(center_price + offset);
            if lower <= Decimal::ZERO {
                return Err(GridBuildError::new(
                    "classic bilateral lower levels must stay positive",
                ));
            }
            lower_levels.push(lower);
            upper_levels.push(upper);
        }

        let plan = GridPlan::bilateral(mode, lower_levels, upper_levels);
        plan.validate_shape()?;
        Ok(plan)
    }

    pub fn classic_bilateral_geometric(
        mode: GridMode,
        center_price: Decimal,
        spacing_bps: u32,
        levels_per_side: u32,
    ) -> Result<GridPlan, GridBuildError> {
        validate_bilateral_mode(mode)?;
        validate_anchor_inputs(center_price, spacing_bps, levels_per_side)?;

        let spacing = spacing_ratio(spacing_bps);
        let lower_ratio = Decimal::ONE - spacing;
        if lower_ratio <= Decimal::ZERO {
            return Err(GridBuildError::new(
                "classic bilateral geometric spacing must keep lower ratio positive",
            ));
        }

        let upper_ratio = Decimal::ONE + spacing;
        let mut lower_levels = Vec::with_capacity(levels_per_side as usize);
        let mut upper_levels = Vec::with_capacity(levels_per_side as usize);
        let mut lower = center_price;
        let mut upper = center_price;

        for _ in 0..levels_per_side {
            lower = normalize_level(lower * lower_ratio);
            upper = normalize_level(upper * upper_ratio);
            if lower <= Decimal::ZERO {
                return Err(GridBuildError::new(
                    "classic bilateral lower levels must stay positive",
                ));
            }
            lower_levels.push(lower);
            upper_levels.push(upper);
        }

        let plan = GridPlan::bilateral(mode, lower_levels, upper_levels);
        plan.validate_shape()?;
        Ok(plan)
    }

    pub fn custom(mode: GridMode, levels: Vec<Decimal>) -> Result<GridPlan, GridBuildError> {
        validate_ordinary_mode(mode)?;

        if levels.is_empty() {
            return Err(GridBuildError::new("grid requires at least one level"));
        }

        if levels.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(GridBuildError::new(
                "custom grid levels must be strictly increasing",
            ));
        }

        let plan = GridPlan::ordinary(mode, levels);
        plan.validate_shape()?;
        Ok(plan)
    }
}

fn validate_anchor_inputs(
    anchor_price: Decimal,
    spacing_bps: u32,
    level_count: u32,
) -> Result<(), GridBuildError> {
    if anchor_price <= Decimal::ZERO {
        return Err(GridBuildError::new("anchor price must be positive"));
    }

    if spacing_bps == 0 {
        return Err(GridBuildError::new("grid spacing must be positive"));
    }

    if level_count == 0 {
        return Err(GridBuildError::new("grid requires at least one level"));
    }

    Ok(())
}

fn validate_ordinary_mode(mode: GridMode) -> Result<(), GridBuildError> {
    if mode.is_ordinary() {
        Ok(())
    } else {
        Err(GridBuildError::new(
            "ordinary fixed-step builder only supports ordinary grid modes",
        ))
    }
}

fn validate_bilateral_mode(mode: GridMode) -> Result<(), GridBuildError> {
    if mode.is_classic_bilateral() {
        Ok(())
    } else {
        Err(GridBuildError::new(
            "classic bilateral builder only supports bilateral grid modes",
        ))
    }
}

fn validate_ordinary_direction(mode: GridMode, levels: &[Decimal]) -> Result<(), GridBuildError> {
    let invalid = match mode {
        GridMode::SpotGrid | GridMode::FuturesLong => {
            levels.windows(2).any(|pair| pair[0] <= pair[1])
        }
        GridMode::FuturesShort => levels.windows(2).any(|pair| pair[0] >= pair[1]),
        GridMode::ClassicBilateralSpot | GridMode::ClassicBilateralFutures => false,
    };

    if invalid {
        return Err(GridBuildError::new(
            "ordinary grid levels must remain strictly monotonic after normalization",
        ));
    }

    Ok(())
}

fn validate_bilateral_levels(
    lower_levels: &[Decimal],
    upper_levels: &[Decimal],
) -> Result<(), GridBuildError> {
    if lower_levels.windows(2).any(|pair| pair[0] <= pair[1]) {
        return Err(GridBuildError::new(
            "classic bilateral lower levels must remain strictly descending",
        ));
    }

    if upper_levels.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(GridBuildError::new(
            "classic bilateral upper levels must remain strictly ascending",
        ));
    }

    Ok(())
}

fn spacing_ratio(spacing_bps: u32) -> Decimal {
    Decimal::new(i64::from(spacing_bps), 4)
}

fn normalize_level(level: Decimal) -> Decimal {
    level.round_dp(8).normalize()
}
