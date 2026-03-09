//! ADC interface for sensor data acquisition.
//!
//! Provides ESP32 ADC configuration and a ring-buffer backed data reader that
//! converts raw ADC values to physical units (femtotesla). The ring buffer is
//! populated via [`AdcReader::load_buffer`] (the production data input path)
//! or by hardware DMA on actual ESP32 targets. On `no_std` the reader would
//! wire directly into the ADC peripheral.

use ruv_neural_core::sensor::SensorType;
use ruv_neural_core::{Result, RuvNeuralError};
use serde::{Deserialize, Serialize};

/// ESP32 ADC input attenuation setting.
///
/// Controls the measurable voltage range on an ADC channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attenuation {
    /// 0 dB — range ~100-950 mV.
    Db0,
    /// 2.5 dB — range ~100-1250 mV.
    Db2_5,
    /// 6 dB — range ~150-1750 mV.
    Db6,
    /// 11 dB — range ~150-2450 mV.
    Db11,
}

impl Attenuation {
    /// Maximum measurable voltage in millivolts for this attenuation.
    pub fn max_voltage_mv(&self) -> u32 {
        match self {
            Attenuation::Db0 => 950,
            Attenuation::Db2_5 => 1250,
            Attenuation::Db6 => 1750,
            Attenuation::Db11 => 2450,
        }
    }
}

/// Configuration for a single ADC channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdcChannel {
    /// ADC channel identifier (0-7 on ESP32).
    pub channel_id: u8,
    /// GPIO pin number this channel is wired to.
    pub gpio_pin: u8,
    /// Input attenuation setting.
    pub attenuation: Attenuation,
    /// Type of sensor connected to this channel.
    pub sensor_type: SensorType,
    /// Gain factor applied during conversion to physical units.
    pub gain: f64,
    /// Offset applied during conversion to physical units.
    pub offset: f64,
}

/// ESP32 ADC configuration for neural sensor readout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdcConfig {
    /// Channels to sample.
    pub channels: Vec<AdcChannel>,
    /// Target sample rate in Hz.
    pub sample_rate_hz: u32,
    /// ADC resolution in bits (12 or 16).
    pub resolution_bits: u8,
    /// Reference voltage in millivolts.
    pub reference_voltage_mv: u32,
    /// Whether DMA transfers are enabled for continuous sampling.
    pub dma_enabled: bool,
}

impl AdcConfig {
    /// Maximum raw ADC value for the configured resolution.
    ///
    /// Clamps the result to `i16::MAX` when `resolution_bits >= 16` to
    /// prevent integer overflow.
    pub fn max_raw_value(&self) -> i16 {
        let bits = self.resolution_bits.min(15);
        ((1u32 << bits) - 1) as i16
    }

    /// Creates a default configuration with a single NV diamond channel.
    pub fn default_single_channel() -> Self {
        Self {
            channels: vec![AdcChannel {
                channel_id: 0,
                gpio_pin: 36,
                attenuation: Attenuation::Db11,
                sensor_type: SensorType::NvDiamond,
                gain: 1.0,
                offset: 0.0,
            }],
            sample_rate_hz: 1000,
            resolution_bits: 12,
            reference_voltage_mv: 3300,
            dma_enabled: false,
        }
    }
}

/// Ring-buffer backed ADC data reader that converts raw ADC values to
/// physical units.
///
/// The internal ring buffer is filled by [`load_buffer`](Self::load_buffer)
/// (the production data input path from DMA or manual sampling) or by
/// [`fill_with_calibration_signal`](Self::fill_with_calibration_signal) for
/// self-test/calibration. On actual ESP32 hardware the DMA controller writes
/// directly into this buffer.
pub struct AdcReader {
    config: AdcConfig,
    buffer: Vec<Vec<i16>>,
    buffer_pos: usize,
}

impl AdcReader {
    /// Create a new reader for the given ADC configuration.
    ///
    /// Allocates a ring buffer with 4096 samples per channel.
    pub fn new(config: AdcConfig) -> Self {
        let num_channels = config.channels.len();
        let buffer_size = 4096;
        let buffer = vec![vec![0i16; buffer_size]; num_channels];
        Self {
            config,
            buffer,
            buffer_pos: 0,
        }
    }

    /// Read `num_samples` from every configured channel, returning values in
    /// femtotesla.
    ///
    /// The outer `Vec` is indexed by channel and the inner `Vec` contains
    /// the converted sample values.
    pub fn read_samples(&mut self, num_samples: usize) -> Result<Vec<Vec<f64>>> {
        if num_samples == 0 {
            return Err(RuvNeuralError::Signal(
                "num_samples must be greater than zero".into(),
            ));
        }

        let num_channels = self.config.channels.len();
        if num_channels == 0 {
            return Err(RuvNeuralError::Sensor(
                "No ADC channels configured".into(),
            ));
        }

        let mut result = Vec::with_capacity(num_channels);
        let buf_len = self.buffer[0].len();

        for (ch_idx, channel) in self.config.channels.iter().enumerate() {
            let mut samples = Vec::with_capacity(num_samples);
            for i in 0..num_samples {
                let pos = (self.buffer_pos + i) % buf_len;
                let raw = self.buffer[ch_idx][pos];
                samples.push(self.to_femtotesla(raw, channel));
            }
            result.push(samples);
        }

        self.buffer_pos = (self.buffer_pos + num_samples) % buf_len;
        Ok(result)
    }

    /// Convert a raw ADC value to femtotesla using the channel's gain and
    /// offset.
    ///
    /// Conversion: `fT = (raw / max_raw) * ref_voltage * gain + offset`
    pub fn to_femtotesla(&self, raw: i16, channel: &AdcChannel) -> f64 {
        let max_raw = self.config.max_raw_value() as f64;
        let voltage_ratio = raw as f64 / max_raw;
        let voltage_mv = voltage_ratio * self.config.reference_voltage_mv as f64;
        voltage_mv * channel.gain + channel.offset
    }

    /// Load raw samples into the internal ring buffer for a given channel.
    ///
    /// This is the production data input path. On real hardware the DMA
    /// controller calls this (or writes directly to the buffer memory) to
    /// deliver new ADC readings. Also used in host-side testing to inject
    /// known waveforms.
    pub fn load_buffer(&mut self, channel_idx: usize, data: &[i16]) -> Result<()> {
        if channel_idx >= self.buffer.len() {
            return Err(RuvNeuralError::ChannelOutOfRange {
                channel: channel_idx,
                max: self.buffer.len().saturating_sub(1),
            });
        }
        let buf_len = self.buffer[channel_idx].len();
        for (i, &val) in data.iter().enumerate() {
            if i >= buf_len {
                break;
            }
            self.buffer[channel_idx][i] = val;
        }
        Ok(())
    }

    /// Returns a reference to the current configuration.
    pub fn config(&self) -> &AdcConfig {
        &self.config
    }

    /// Resets the buffer read position to zero.
    pub fn reset(&mut self) {
        self.buffer_pos = 0;
    }

    /// Fill all channels with a known sinusoidal calibration signal for
    /// self-test and gain verification.
    ///
    /// Writes a full-scale sine wave at the given frequency into every
    /// channel's ring buffer. After calling this, [`read_samples`](Self::read_samples)
    /// will return the calibration waveform converted to femtotesla, which
    /// can be compared against the expected amplitude to verify the gain
    /// and offset calibration.
    ///
    /// # Arguments
    /// * `frequency_hz` - Frequency of the calibration sine wave.
    ///
    /// # Example
    /// ```
    /// # use ruv_neural_esp32::adc::{AdcConfig, AdcReader};
    /// let config = AdcConfig::default_single_channel();
    /// let mut reader = AdcReader::new(config);
    /// reader.fill_with_calibration_signal(10.0);
    /// let data = reader.read_samples(100).unwrap();
    /// // data now contains a 10 Hz sine converted to fT
    /// ```
    pub fn fill_with_calibration_signal(&mut self, frequency_hz: f64) {
        let buf_len = self.buffer[0].len();
        let max_raw = self.config.max_raw_value();
        let sample_rate = self.config.sample_rate_hz as f64;

        for ch_idx in 0..self.buffer.len() {
            for i in 0..buf_len {
                let t = i as f64 / sample_rate;
                // Sine wave at ~90% of full scale to avoid clipping
                let value = 0.9 * (max_raw as f64)
                    * (2.0 * std::f64::consts::PI * frequency_hz * t).sin();
                self.buffer[ch_idx][i] = value.round() as i16;
            }
        }
        self.buffer_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_femtotesla_known_value() {
        let config = AdcConfig {
            channels: vec![AdcChannel {
                channel_id: 0,
                gpio_pin: 36,
                attenuation: Attenuation::Db11,
                sensor_type: SensorType::NvDiamond,
                gain: 2.0,
                offset: 10.0,
            }],
            sample_rate_hz: 1000,
            resolution_bits: 12,
            reference_voltage_mv: 3300,
            dma_enabled: false,
        };
        let reader = AdcReader::new(config);
        let channel = &reader.config().channels[0];

        // raw = 2048, max = 4095, ratio = 0.5001..., voltage = ~1650.4 mV
        // fT = 1650.4 * 2.0 + 10.0 = ~3310.8
        let ft = reader.to_femtotesla(2048, channel);
        let expected = (2048.0 / 4095.0) * 3300.0 * 2.0 + 10.0;
        assert!((ft - expected).abs() < 1e-6, "got {ft}, expected {expected}");
    }

    #[test]
    fn test_read_samples_length() {
        let config = AdcConfig::default_single_channel();
        let mut reader = AdcReader::new(config);
        let result = reader.read_samples(100).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 100);
    }

    #[test]
    fn test_load_buffer_and_read() {
        let config = AdcConfig::default_single_channel();
        let mut reader = AdcReader::new(config);
        let data: Vec<i16> = (0..10).collect();
        reader.load_buffer(0, &data).unwrap();
        let result = reader.read_samples(10).unwrap();
        // Values should be monotonically increasing since raw values are 0..10
        for i in 1..10 {
            assert!(result[0][i] > result[0][i - 1]);
        }
    }

    #[test]
    fn test_read_zero_samples_error() {
        let config = AdcConfig::default_single_channel();
        let mut reader = AdcReader::new(config);
        assert!(reader.read_samples(0).is_err());
    }

    #[test]
    fn test_attenuation_max_voltage() {
        assert_eq!(Attenuation::Db0.max_voltage_mv(), 950);
        assert_eq!(Attenuation::Db11.max_voltage_mv(), 2450);
    }
}
