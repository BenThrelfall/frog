use std::{cell::RefCell, f64::consts::PI};

use rand::Rng;
pub use rand_distr::{Distribution, Normal, Uniform};
use serde::{Deserialize, Serialize};

use crate::{units::*, SNR_MAX, SNR_MIN};

use super::{data_structs::Transmission, Context};

/// Minimum SNR required for successful demodulation and reading of recieved transmission.
/// Based on tables from LoRa datasheets:
///
/// Semtech Corporation. 2024. SX1261/2 Datasheet. Accessed 30/04/2025. Available at
/// [Link 1](https://www.semtech.com/products/wireless-rf/lora-connect/sx1262)
/// [Link 2](https://semtech.my.salesforce.com/sfc/p/#E0000000JelG/a/RQ000008nKCH/hp2iKwMDKWl34g1D3LBf_zC7TGBRIo2ff5LMnS8r19s)
///
/// Semtech Corporation. 2020. SX1276/77/78/79 - 137 MHz to 1020 MHz Low Power Long Range Transceiver. Accessed 30/04/2025.
/// Available at
/// [Link 1](https://www.semtech.com/products/wireless-rf/lora-connect/sx1278)
/// [Link 2](https://semtech.my.salesforce.com/sfc/p/#E0000000JelG/a/2R0000001Rc1/QnUuV9TviODKUgt_rpBlPz.EZA_PNK7Rpi8HA5..Sbo)
#[inline]
fn snr_read_threshold(sf: i32) -> Dbf {
    Dbf::from_db_value(-2.5 * (sf as f64) + 10.0)
}

/// Assumed to be the same as the read threshold for now.
/// See [`snr_read_threshold`].
#[inline]
fn snr_detect_threshold(sf: i32) -> Dbf {
    Dbf::from_db_value(-2.5 * (sf as f64) + 10.0)
}

const MIN_RECEIVED_POWER: Db<Power> = Dbm::from_dbm(-10000.0);

macro_rules! pathloss_model {
    ($($variant:ident),+) => {

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        pub enum PathlossModel {
            $(
                $variant($variant),
            )*
        }

        impl PathlossModel {
            pub fn power_at_reciever(
                &self,
                sender_power: Db<Power>,
                wave_length: Length,
                distance: Length,
            ) -> Db<Power>{
                match self {
                    $(
                        PathlossModel::$variant(inner) => inner.power_at_reciever(sender_power, wave_length, distance),
                    )*
                }
            }
        }

        $(
        impl From<$variant> for PathlossModel {
            fn from(value: $variant) -> Self {
                PathlossModel::$variant(value)
            }
        }
        )*


    };


}

pathloss_model!(NoPathloss, AdjustedFreeSpacePathLoss, LinearPathLoss);

trait ImplPathlossModel {
    fn power_at_reciever(
        &self,
        sender_power: Db<Power>,
        wave_length: Length,
        distance: Length,
    ) -> Db<Power>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoPathloss;
impl ImplPathlossModel for NoPathloss {
    fn power_at_reciever(
        &self,
        sender_power: Db<Power>,
        _wave_length: Length,
        _distance: Length,
    ) -> Db<Power> {
        sender_power
    }
}

/// Standard free space path loss with custom distance exponent
/// <https://en.wikipedia.org/wiki/Free-space_path_loss>
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdjustedFreeSpacePathLoss {
    /// Normally 2.0
    pub distance_exponent: f64,

    /// Adjustment term representing any other
    /// losses or gains in the system
    /// real or effective.
    pub other_loss_or_gain: Db<f64>,
}

impl AdjustedFreeSpacePathLoss {
    pub fn new(distance_exponent: f64, other_loss_or_gain: Db<f64>) -> Self {
        Self {
            distance_exponent,
            other_loss_or_gain,
        }
    }
}

pub fn adjusted_free_space_path_loss(exponent: f64) -> AdjustedFreeSpacePathLoss {
    AdjustedFreeSpacePathLoss {
        distance_exponent: exponent,
        other_loss_or_gain: 0.0.into(),
    }
}

pub fn free_space_path_loss() -> AdjustedFreeSpacePathLoss {
    AdjustedFreeSpacePathLoss {
        distance_exponent: 2.0,
        other_loss_or_gain: 0.0.into(),
    }
}

impl ImplPathlossModel for AdjustedFreeSpacePathLoss {
    fn power_at_reciever(
        &self,
        sender_power: Db<Power>,
        wave_length: Length,
        distance: Length,
    ) -> Db<Power> {
        let loss = self.distance_exponent * Db::from_unit(distance)
            + 2.0 * Dbf::from_unit(4.0 * PI)
            - 2.0 * Db::from_unit(wave_length)
            + self.other_loss_or_gain;

        let reciever_power = sender_power - loss;
        reciever_power
    }
}

/// Pathloss where dB loss increases linearly with distance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LinearPathLoss {
    pub loss_rate: DbPerLength,
}
impl ImplPathlossModel for LinearPathLoss {
    fn power_at_reciever(
        &self,
        sender_power: Db<Power>,
        _wave_length: Length,
        distance: Length,
    ) -> Db<Power> {
        sender_power - (self.loss_rate * distance)
    }
}

/// Standard log path loss with reference loss value at reference distance
/// and a pathloss exponent. This model is useful because many considerations
/// are implicitly taken into account in the reference measurement if done empirically
///
/// This currently doesn't fully make sense with the way the rest of the sim is set up.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogPathLoss {
    pub reference_loss: Dbf,
    pub reference_distance: Length,
    pub exponent: f64,
}
impl ImplPathlossModel for LogPathLoss {
    fn power_at_reciever(
        &self,
        sender_power: Db<Power>,
        _wave_length: Length,
        distance: Length,
    ) -> Db<Power> {
        let loss = self.reference_loss
            + (self.exponent * Db::from_unit(distance / self.reference_distance));
        sender_power - loss
    }
}
impl Default for LogPathLoss {
    fn default() -> Self {
        Self {
            reference_loss: Db::from_unit(127.41),
            reference_distance: Length::from_metres(40.0),
            exponent: 2.08,
        }
    }
}

pub enum TransmissionResult {
    Success { snr: Db<f64> },
    TooWeak,
    Blocked { blocker_id: u32 },
}

macro_rules! transmission_model {
    ($($variant:ident),+) => {

        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        pub enum TransmissionModel {
            $(
                $variant($variant),
            )*
        }

        impl TransmissionModel{

            pub fn detected_at(
                &self,
                sim: &Context,
                at_node: usize,
                transmission: &Transmission,
            ) -> bool{
                match self {
                    $(
                        TransmissionModel::$variant(inner) => inner.detected_at(sim, at_node, transmission),
                    )*
                }
            }

            /// Returns a [TransmissionResult] indicating if the target transmission can be recieved at the node.
            pub fn reception_at(
                &self,
                sim: &Context,
                at_node: usize,
                target: &Transmission,
            ) -> TransmissionResult{
                match self {
                        $(
                            TransmissionModel::$variant(inner) => inner.reception_at(sim, at_node, target),
                        )*
                }
            }

            // Returns true of the node can detect a broadcast (even under blocking interference) and false otherwise.
            pub fn detecting_any_at(&self, sim: &Context, at_node: usize) -> bool{
                match self {
                    $(
                        TransmissionModel::$variant(inner) => inner.detecting_any_at(sim, at_node),
                    )*
                }
            }
        }

        $(
        impl From<$variant> for TransmissionModel {
            fn from(value: $variant) -> Self {
                TransmissionModel::$variant(value)
            }
        }
        )*
    };
}

type PairWiseNormal = PairWiseCaptureEffect<Normal<f64>>;
type PairWiseNone = PairWiseCaptureEffect<NoneDist>;
type PairWiseUniform = PairWiseCaptureEffect<Uniform<f64>>;

transmission_model!(PairWiseNormal, PairWiseNone, PairWiseUniform);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoneDist;
impl Distribution<f64> for NoneDist {
    fn sample<R: Rng + ?Sized>(&self, _rng: &mut R) -> f64 {
        0.0
    }
}

trait ImplTransmissionModel {
    fn detected_at(&self, sim: &Context, at_node: usize, transmission: &Transmission) -> bool;

    /// Returns a [TransmissionResult] indicating if the target transmission can be recieved at the node.
    fn reception_at(
        &self,
        sim: &Context,
        at_node: usize,
        target: &Transmission,
    ) -> TransmissionResult;

    // Returns true of the node can detect a broadcast (even under blocking interference) and false otherwise.
    fn detecting_any_at(&self, sim: &Context, at_node: usize) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairWiseStore<C> {
    pub path_loss: PathlossModel,
    pub noise_temp: Temperature,
    pub random_fading: C,
}

impl<C> From<PairWiseCaptureEffect<C>> for PairWiseStore<C>
where
    C: Clone,
{
    fn from(value: PairWiseCaptureEffect<C>) -> Self {
        PairWiseStore {
            path_loss: value.path_loss,
            noise_temp: value.noise_temp,
            random_fading: value.random_fading,
        }
    }
}

impl<C> From<PairWiseStore<C>> for PairWiseCaptureEffect<C>
where
    C: Clone,
{
    fn from(value: PairWiseStore<C>) -> Self {
        PairWiseCaptureEffect {
            path_loss: value.path_loss,
            noise_temp: value.noise_temp,
            random_fading: value.random_fading,
            cached_power_at: Default::default(),
            db_noise_energy: Db::from_unit(BOLTZMANN * value.noise_temp),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(from = "PairWiseStore<C>")]
#[serde(into = "PairWiseStore<C>")]
pub struct PairWiseCaptureEffect<C>
where
    C: Clone,
{
    pub path_loss: PathlossModel,

    /// The effective noise temperature at each node.
    /// Setting this to [`Temperature::ROOM_TEMP`] is reasonable.
    pub noise_temp: Temperature,

    pub random_fading: C,

    #[serde(skip)]
    cached_power_at: RefCell<Vec<Vec<Option<Db<Power>>>>>,

    #[serde(default = "path")]
    db_noise_energy: Db<Energy>,
}

impl Default for PairWiseCaptureEffect<NoneDist> {
    fn default() -> Self {
        Self::new(
            free_space_path_loss().into(),
            Temperature::ROOM_TEMP,
            NoneDist,
        )
    }
}

// Builder like methods
impl<T> PairWiseCaptureEffect<T>
where
    T: Clone + Distribution<f64>,
{
    pub fn with_pathloss(self, pathloss: PathlossModel) -> Self {
        Self::new(pathloss, self.noise_temp, self.random_fading)
    }

    pub fn with_fading<C>(self, fading: C) -> PairWiseCaptureEffect<C>
    where
        C: Clone + Distribution<f64>,
    {
        PairWiseCaptureEffect::new(self.path_loss, self.noise_temp, fading)
    }
}

impl<C> ImplTransmissionModel for PairWiseCaptureEffect<C>
where
    C: Distribution<f64> + Clone,
{
    fn reception_at(
        &self,
        sim: &Context,
        at_node: usize,
        transmission: &Transmission,
    ) -> TransmissionResult {
        /// From Croce, D. et al. (2018)
        /// ‘Impact of lora imperfect orthogonality: Analysis of link-level performance’,
        /// IEEE Communications Letters, 22(4), pp. 796–799. https://doi.org/10.1109/LCOMM.2018.2797057.
        const SIR_THRESHOLDS: [[f64; 6]; 6] = [
            [1.0, -8.0, -9.0, -9.0, -9.0, -9.0],
            [-11.0, 1.0, -11.0, -12.0, -13.0, -13.0],
            [-15.0, -13.0, 1.0, -13.0, -14.0, -15.0],
            [-19.0, -18.0, -17.0, 1.0, -17.0, -18.0],
            [-22.0, -22.0, -21.0, -20.0, 1.0, -20.0],
            [-25.0, -25.0, -25.0, -24.0, -23.0, 1.0],
        ];

        let target_power = self.power_at(sim, at_node, transmission);
        let snr = target_power - self.noise_power(transmission.bandwidth);

        if snr < snr_read_threshold(transmission.sf) {
            return TransmissionResult::TooWeak;
        }

        let maybe_blocker_id = sim
            .em_field
            .iter()
            .rev()
            .take_while(|x| x.end_time >= transmission.start_time)
            .find(|x| {
                if x.id == transmission.id {
                    return false;
                }
                // Assumes you can never transmit and recieve at the same time
                if x.transmitter_id == at_node {
                    return true;
                }
                if x.carrier_band != transmission.carrier_band {
                    return false;
                }

                // Uses distance to in power_at
                let blocker_power = self.power_at(sim, at_node, x);

                let signal_interference_ratio = target_power - blocker_power;
                let threshold: Db<f64> =
                    SIR_THRESHOLDS[(transmission.sf - 7) as usize][(x.sf - 7) as usize].into();

                signal_interference_ratio <= threshold
            })
            .map(|x| x.id);

        if let Some(id) = maybe_blocker_id {
            TransmissionResult::Blocked { blocker_id: id }
        } else {
            TransmissionResult::Success {
                snr: snr.map(|x| x.clamp(SNR_MIN, SNR_MAX)),
            }
        }
    }

    fn detected_at(&self, sim: &Context, at_node: usize, transmission: &Transmission) -> bool {
        if sim.settings.carrier_band != transmission.carrier_band {
            return false;
        }

        let power = self.power_at(sim, at_node, transmission);
        let snr = power - self.noise_power(transmission.bandwidth);

        snr >= snr_detect_threshold(transmission.sf)
    }

    fn detecting_any_at(&self, sim: &Context, at_node: usize) -> bool {
        for transmission in sim.active_transmissions() {
            if self.detected_at(sim, at_node, transmission) {
                return true;
            }
        }

        false
    }
}

impl<C> PairWiseCaptureEffect<C>
where
    C: Distribution<f64> + Clone,
{
    pub fn new(path_loss: PathlossModel, noise_temp: Temperature, random_fading: C) -> Self {
        Self {
            path_loss,
            noise_temp,
            random_fading,
            cached_power_at: Default::default(),
            db_noise_energy: Db::from_unit(BOLTZMANN * noise_temp),
        }
    }

    /// Returns the recieved power at the given node from the given transmission in dBm
    fn power_at(&self, sim: &Context, at_node: usize, target: &Transmission) -> Db<Power> {
        // We cache the value because it should not have different random effects
        // for the same transmission at node pair.

        let mut cache = self.cached_power_at.borrow_mut();

        //cache.get(&(target.id, at_node))
        if let Some(val) = cache
            .get(target.id as usize)
            .and_then(|inner| inner.get(at_node).copied().flatten())
        {
            val
        } else {
            let Some(distance) =
                sim.graph
                    .distance_to(sim.sim_time, target.transmitter_id, at_node)
            else {
                return MIN_RECEIVED_POWER;
            };

            let target_power = self.path_loss.power_at_reciever(
                target.power,
                target.carrier_band.wave_length(),
                distance,
            );

            let fading = self.random_fading.sample(&mut sim.rng.borrow_mut());

            let final_power = target_power + Dbf::from_db_value(fading);

            let index = target.id as usize;
            while cache.len() <= index {
                cache.push(vec![None; sim.graph.len()]);
            }

            cache[index][at_node] = Some(final_power);

            final_power
        }
    }

    fn noise_power(&self, bandwidth: Frequency) -> Db<Power> {
        let db_bandwidth: Db<Frequency> = match bandwidth.kHz() {
            249.0..251.0 => Db::from(53.9794000867),
            _ => Db::from_unit(bandwidth),
        };

        self.db_noise_energy + db_bandwidth
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_close,
        units::{Dbf, Dbm, Frequency, Length},
    };

    use super::{
        snr_detect_threshold, snr_read_threshold, AdjustedFreeSpacePathLoss, ImplPathlossModel,
    };

    #[test]
    fn standard_free_space() {
        let model = AdjustedFreeSpacePathLoss {
            distance_exponent: 2.0,
            other_loss_or_gain: 0.0.into(),
        };

        let in_power = Dbm::from_dbm(22.0);
        let result = model.power_at_reciever(
            in_power,
            Frequency::from_MHz(868.0).light_wavelength(),
            Length::from_metres(3000.0),
        );

        let reference = in_power - Dbf::from_db_value(100.76321);

        assert_close(result, reference);
    }

    #[test]
    fn quasi_free_space() {
        let model = AdjustedFreeSpacePathLoss {
            distance_exponent: 3.5,
            other_loss_or_gain: 0.0.into(),
        };

        let in_power = Dbm::from_dbm(22.0);
        let result = model.power_at_reciever(
            in_power,
            Frequency::from_MHz(868.0).light_wavelength(),
            Length::from_metres(3000.0),
        );

        let reference = in_power - Dbf::from_db_value(152.92003);

        assert_close(result, reference);
    }

    #[test]
    fn snr_thresholds() {

        // Expected values from sf 5 to 12
        let expected =
            [-2.5, -5.0, -7.5, -10.0, -12.5, -15.0, -17.5, -20.0].map(|n| Dbf::from_db_value(n));

        for sf in 5..=12 {
            let read_calculated = snr_read_threshold(sf);
            let detect_calculated = snr_detect_threshold(sf);

            assert_eq!(expected[(sf - 5) as usize], read_calculated);
            assert_eq!(expected[(sf - 5) as usize], detect_calculated);
        }
    }
}
