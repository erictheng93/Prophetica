use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChaosScenario {
    EnginePodCrash,
    HostPowerLoss,
    RegionFailure,
    WalDiskFailure,
    KafkaClusterFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChaosRtoViolation {
    pub scenario: ChaosScenario,
    pub target: Duration,
    pub measured: Duration,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChaosRtoHarness;

impl ChaosRtoHarness {
    /// Records an RTO measurement and validates it against SPEC-008 §4.2.
    ///
    /// # Errors
    ///
    /// Returns [`ChaosRtoViolation`] when the measured RTO exceeds the scenario target.
    pub fn record_rto(
        &self,
        scenario: ChaosScenario,
        measured: Duration,
    ) -> Result<(), ChaosRtoViolation> {
        let target = scenario.rto_target();
        if measured <= target {
            return Ok(());
        }

        Err(ChaosRtoViolation {
            scenario,
            target,
            measured,
        })
    }
}

impl ChaosScenario {
    #[must_use]
    pub const fn rto_target(self) -> Duration {
        match self {
            Self::EnginePodCrash => Duration::from_secs(30),
            Self::HostPowerLoss | Self::RegionFailure | Self::KafkaClusterFailure => {
                Duration::from_secs(5 * 60)
            }
            Self::WalDiskFailure => Duration::from_secs(10 * 60),
        }
    }
}
