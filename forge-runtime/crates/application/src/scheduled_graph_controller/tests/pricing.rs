use super::{
    ScheduledGraphControllerClock, ScheduledGraphControllerPricingSource,
    ScheduledGraphControllerPricingSourceError,
};

pub(super) struct InputTimeClock;

impl ScheduledGraphControllerClock for InputTimeClock {
    fn now_ms(&self) -> u64 {
        0
    }
}

pub(super) struct StaticPricing(pub(super) String);

impl ScheduledGraphControllerPricingSource for StaticPricing {
    fn read_pricing_json(&self) -> Result<String, ScheduledGraphControllerPricingSourceError> {
        Ok(self.0.clone())
    }
}
