use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, IntCounter, IntCounterVec, IntGauge, Opts,
    Registry, TextEncoder,
};
use std::{
    error,
    time::{Duration, Instant},
};

pub const METRICS_TTL: Duration = Duration::from_millis(10_000);

pub struct Metrics {
    encoder: TextEncoder,
    registry: Registry,
    pub last_update: Instant,

    energy_delivered_joules_total: CounterVec,
    energy_received_joules_total: CounterVec,
    energy_tariff: IntGauge,
    power_delivered_watts: Gauge,
    power_received_watts: Gauge,
    power_failures_total: IntCounter,
    power_long_failures_total: IntCounter,
    phase_voltage_sags_total: IntCounterVec,
    phase_voltage_swells_total: IntCounterVec,
    phase_voltage_volts: GaugeVec,
    phase_current_amperes: GaugeVec,
    phase_active_power_positive_watts: GaugeVec,
    phase_active_power_negative_watts: GaugeVec,
    gas_delivered_cubic_meters_total: Counter,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let energy_delivered_joules_total = CounterVec::new(
            Opts::new(
                "energy_delivered_joules_total",
                "The amount of energy delivered to client in joules",
            ),
            &["tariff"],
        )
        .unwrap();
        registry
            .register(Box::new(energy_delivered_joules_total.clone()))
            .unwrap();

        let energy_received_joules_total = CounterVec::new(
            Opts::new(
                "energy_received_joules_total",
                "The amount of energy delivered by client in joules",
            ),
            &["tariff"],
        )
        .unwrap();
        registry
            .register(Box::new(energy_received_joules_total.clone()))
            .unwrap();

        let energy_tariff =
            IntGauge::with_opts(Opts::new("energy_tariff", "The currently active tariff")).unwrap();
        registry.register(Box::new(energy_tariff.clone())).unwrap();

        let power_delivered_watts = Gauge::with_opts(Opts::new(
            "power_delivered_watts",
            "The amount of power that is currently being delivered to client in Watts",
        ))
        .unwrap();
        registry
            .register(Box::new(power_delivered_watts.clone()))
            .unwrap();

        let power_received_watts = Gauge::with_opts(Opts::new(
            "power_received_watts",
            "The amount of power that is currently being delivered by client in Watts",
        ))
        .unwrap();
        registry
            .register(Box::new(power_received_watts.clone()))
            .unwrap();

        // power_failures counter
        let power_failures_total = IntCounter::with_opts(Opts::new(
            "power_failures_total",
            "Number of power failures in any phase",
        ))
        .unwrap();
        registry
            .register(Box::new(power_failures_total.clone()))
            .unwrap();

        // power_long_failures counter
        let power_long_failures_total = IntCounter::with_opts(Opts::new(
            "power_long_failures_total",
            "Number of long power failures in any phase",
        ))
        .unwrap();
        registry
            .register(Box::new(power_long_failures_total.clone()))
            .unwrap();

        // voltage_sags counter {line}
        let phase_voltage_sags_total = IntCounterVec::new(
            Opts::new(
                "phase_voltage_sags_total",
                "Number of voltage sags in specified phase",
            ),
            &["phase"],
        )
        .unwrap();
        registry
            .register(Box::new(phase_voltage_sags_total.clone()))
            .unwrap();

        // voltage_swells counter {line}
        let phase_voltage_swells_total = IntCounterVec::new(
            Opts::new(
                "phase_voltage_swells_total",
                "Number of voltage swells in specified phase",
            ),
            &["phase"],
        )
        .unwrap();
        registry
            .register(Box::new(phase_voltage_swells_total.clone()))
            .unwrap();

        // voltage gauge {line}
        let phase_voltage_volts = GaugeVec::new(
            Opts::new(
                "phase_voltage_volts",
                "Instantaneous voltage in specified phase in Volts",
            ),
            &["phase"],
        )
        .unwrap();
        registry
            .register(Box::new(phase_voltage_volts.clone()))
            .unwrap();

        // current gauge {line}
        let phase_current_amperes = GaugeVec::new(
            Opts::new(
                "phase_current_amperes",
                "Instantaneous current in specified phase in Ampères",
            ),
            &["phase"],
        )
        .unwrap();
        registry
            .register(Box::new(phase_current_amperes.clone()))
            .unwrap();

        // active_power_positive gauge {line}
        let phase_active_power_positive_watts = GaugeVec::new(
            Opts::new(
                "phase_active_power_positive_watts",
                "Instantaneous active power (+P) in specified phase in Watts",
            ),
            &["phase"],
        )
        .unwrap();
        registry
            .register(Box::new(phase_active_power_positive_watts.clone()))
            .unwrap();

        // active_power_negative gauge {line}
        let phase_active_power_negative_watts = GaugeVec::new(
            Opts::new(
                "phase_active_power_negative_watts",
                "Instantaneous active power (-P) in specified phase in Watts",
            ),
            &["phase"],
        )
        .unwrap();
        registry
            .register(Box::new(phase_active_power_negative_watts.clone()))
            .unwrap();

        // gas_delivered counter (m3)
        let gas_delivered_cubic_meters_total = Counter::with_opts(Opts::new(
            "gas_delivered_cubic_meters_total",
            "Amount of natural gas delivered to client in cubic meters",
        ))
        .unwrap();
        registry
            .register(Box::new(gas_delivered_cubic_meters_total.clone()))
            .unwrap();

        Metrics {
            encoder: TextEncoder::new(),
            registry,
            energy_delivered_joules_total,
            energy_received_joules_total,
            energy_tariff,
            power_delivered_watts,
            power_received_watts,
            power_failures_total,
            power_long_failures_total,
            phase_voltage_sags_total,
            phase_voltage_swells_total,
            phase_voltage_volts,
            phase_current_amperes,
            phase_active_power_negative_watts,
            phase_active_power_positive_watts,
            gas_delivered_cubic_meters_total,
            last_update: Instant::now() - METRICS_TTL,
        }
    }

    pub fn update(&mut self, state: &dsmr5::state::State) {
        for (i, reading) in state.meterreadings.iter().enumerate() {
            if let Some(energy_delivered_kwh) = reading.to {
                let counter = self
                    .energy_delivered_joules_total
                    .with_label_values(&[&(i + 1).to_string()]);
                counter.inc_by(energy_delivered_kwh * 3600000.0 - counter.get());
            }

            if let Some(energy_received_kwh) = reading.by {
                let counter = self
                    .energy_received_joules_total
                    .with_label_values(&[&(i + 1).to_string()]);
                counter.inc_by(energy_received_kwh * 3600000.0 - counter.get());
            }
        }

        if let Some(energy_tariff) = state.tariff_indicator {
            self.energy_tariff.set(i64::from_be_bytes([
                0,
                0,
                0,
                0,
                0,
                0,
                energy_tariff[0],
                energy_tariff[1],
            ]));
        }

        if let Some(power_delivered) = state.power_delivered {
            self.power_delivered_watts.set(power_delivered * 1000.0);
        }

        if let Some(power_received) = state.power_received {
            self.power_received_watts.set(power_received * 1000.0);
        }

        if let Some(power_failures) = state.power_failures {
            self.power_failures_total
                .inc_by(power_failures - self.power_failures_total.get());
        }

        if let Some(long_power_failures) = state.long_power_failures {
            self.power_long_failures_total
                .inc_by(long_power_failures - self.power_long_failures_total.get());
        }

        for (i, line) in state.lines.iter().enumerate() {
            if let Some(voltage_sags) = line.voltage_sags {
                let counter = self
                    .phase_voltage_sags_total
                    .with_label_values(&[&(i + 1).to_string()]);
                counter.inc_by(voltage_sags - counter.get());
            }

            if let Some(voltage_swells) = line.voltage_swells {
                let counter = self
                    .phase_voltage_swells_total
                    .with_label_values(&[&(i + 1).to_string()]);
                counter.inc_by(voltage_swells - counter.get());
            }

            if let Some(voltage) = line.voltage {
                self.phase_voltage_volts
                    .with_label_values(&[&(i + 1).to_string()])
                    .set(voltage);
            }

            if let Some(current) = line.current {
                self.phase_current_amperes
                    .with_label_values(&[&(i + 1).to_string()])
                    .set(current as f64);
            }

            if let Some(active_power_positive) = line.active_power_plus {
                self.phase_active_power_positive_watts
                    .with_label_values(&[&(i + 1).to_string()])
                    .set(active_power_positive * 1000.0);
            }

            if let Some(active_power_negative) = line.active_power_neg {
                self.phase_active_power_negative_watts
                    .with_label_values(&[&(i + 1).to_string()])
                    .set(active_power_negative * 1000.0);
            }
        }

        if let Some(gas_slave) = state
            .slaves
            .iter()
            .find(|slave| slave.device_type == Some(3))
        {
            if let Some((_, reading)) = gas_slave.meter_reading {
                self.gas_delivered_cubic_meters_total
                    .inc_by(reading - self.gas_delivered_cubic_meters_total.get());
            }
        }

        self.last_update = Instant::now();
    }

    pub fn encode(&self) -> Result<String, Box<dyn error::Error>> {
        let metrics = self.registry.gather();
        let mut buffer = vec![];
        self.encoder.encode(&metrics, &mut buffer)?;

        Ok(String::from_utf8(buffer)?)
    }
}
