//! Radio network simulation and analysis tools.
//!
//! ## Custom Node Models
//! Users can implement custom node models (for custom routing algorithms) using [`node::NodeModel`] and [`simulation::Context`]
//! with little knowledge of how the underlying simulator works. See the documentation for each for guidance on how to do this.

pub mod analysis;
pub mod node;
pub mod node_location;
pub mod sim_file;
pub mod simulation;
pub mod units;
pub mod verification;
pub mod scenario;
mod utility;

use std::fmt::Debug;

use simulation::data_structs::NodeSettings;
use units::*;

// LoRA consts
// RadioInterface::getCWsize says these are the minimum and maximum
// values for SNR that LoRA will give.
const SNR_MIN: f64 = -15.0;
const SNR_MAX: f64 = 20.0;

/// Based off the firmware code RadioInterface::getPacketTime
/// I can't find the original source for this formula but the same thing is used [here](https://www.rfwireless-world.com/calculators/LoRaWAN-Airtime-calculator.html)
///
/// * `payload_size` - size of the header and packet body in bytes.
pub fn calculate_air_time(payload_size: i32, radio_setting: &NodeSettings) -> Time {
    /// Is the header disabled. Refers to the LoRA Phys header not meshtastic header.
    /// This is a number not a bool for convenience.
    const HEAD_DISABLE: i32 = 0;
    const PREAMBLE_LEN: f64 = 16.0;

    let coding_rate: f64 = radio_setting.coding_rate as f64;

    let sf = radio_setting.sf;
    let symbol_time = 2f64.powi(sf) / radio_setting.bandwidth;

    let low_data_mode = symbol_time > Time::from_milis(16.0);

    let preamble_time = (PREAMBLE_LEN + 4.25) * symbol_time;

    // What all the magic numbers in this mean is a mystery to me. Looking through quite a number of papers has not helped
    let probably_number_of_bits_before_coding =
        (8 * payload_size - 4 * sf + 28 + 16 - 20 * HEAD_DISABLE) as f64;
    let adjusted_sf = if low_data_mode { sf - 2 } else { sf } as f64;

    let payload_symbols = 8.0
        + ((probably_number_of_bits_before_coding * coding_rate) / (4.0 * adjusted_sf))
            .ceil()
            .max(0.0);
    let payload_time = payload_symbols * symbol_time;
    let packet_time = preamble_time + payload_time;

    packet_time
}

/// Checks two values are within 0.001% of each other.
#[allow(unused)]
fn assert_close<T>(a: T, b: T)
where
    T: Into<f64> + Copy + Debug,
{
    let float_a: f64 = a.into();
    let float_b: f64 = b.into();

    if float_a == 0. || float_b == 0. {
        assert!(float_a == float_b, "{a:?} and {b:?} are not close.");
        return;
    }

    let percent_diff = (float_a - float_b).abs() / float_a.abs();

    assert!(percent_diff < 0.00001, "{a:?} and {b:?} are not close.");
}

#[cfg(test)]
mod tests {
    use crate::{assert_close, units::Length};

    #[test]
    fn test_assert_close_pos() {
        assert_close(10.0, 10.0);
        assert_close(Length::from_metres(200.002), Length::from_metres(200.001));
    }

    #[test]
    fn test_assert_close_neg() {
        let result = std::panic::catch_unwind(|| assert_close(10.0, 11.0));
        assert!(result.is_err());
        let result = std::panic::catch_unwind(|| {
            assert_close(Length::from_metres(10231.0), Length::from_metres(10231.15))
        });
        assert!(result.is_err());
        let result = std::panic::catch_unwind(|| assert_close(0.01, 0.002));
        assert!(result.is_err());
    }
}
