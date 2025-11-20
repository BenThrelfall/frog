//! Verifications / tests to be run on simulation results to make sure the simulator is working correctly.
//! Each public function, other than [`verify_all`], represents some property that should hold for all simulation results.

use crate::{analysis::CompleteAnalysis, simulation::data_structs::LogContent};

pub fn verify_all(analysis: &CompleteAnalysis) -> bool {
    no_overlapping_transmission(analysis)
        && no_overlapping_reception(analysis)
        && no_transmission_and_reception_at_same_time(analysis)
}

/// No node can make more than one transmission at a time.
///
/// For all non-equal transmissions if they have the same sender they cannot overlap.
pub fn no_overlapping_transmission(analysis: &CompleteAnalysis) -> bool {
    for trans_a in analysis.transmissions.iter() {
        for trans_b in analysis.transmissions.iter() {
            if trans_a.id != trans_b.id
                && trans_a.transmitter_id == trans_b.transmitter_id
                && trans_a.overlaps(trans_b)
            {
                eprintln!("Overlapping transmission");
                eprintln!("= First = {trans_a:#?} \n = Second = \n {trans_b:#?}");
                return false;
            }
        }
    }

    return true;
}

/// No node can receive more than one transmission at a time.
pub fn no_overlapping_reception(analysis: &CompleteAnalysis) -> bool {
    let iter: Vec<_> = analysis
        .sim_events
        .iter()
        .filter_map(|x| match x.content {
            LogContent::TransmissionReceived {
                receiver_id,
                transmission_id,
            } => Some((
                receiver_id,
                analysis
                    .transmissions
                    .iter()
                    .find(|x| x.id == transmission_id)
                    .unwrap(),
            )),
            _ => None,
        })
        .collect();

    for (node_a, trans_a) in iter.iter() {
        for (node_b, trans_b) in iter.iter() {
            if node_a == node_b && trans_a.id != trans_b.id && trans_a.overlaps(trans_b) {
                eprintln!("Overlapping reception at {node_a} ({node_b}");
                eprintln!("= First = {trans_a:#?} \n = Second = \n {trans_b:#?}");
                return false;
            }
        }
    }

    return true;
}

/// No node can make a transmission and receive a transmission at the same time.
pub fn no_transmission_and_reception_at_same_time(analysis: &CompleteAnalysis) -> bool {
    let overlaps = overlapping_transmissions(analysis);
    let recievers = transmission_recievers(analysis);

    for (n, m) in overlaps.iter() {
        let first = &analysis.transmissions[*n];
        let second = &analysis.transmissions[*m];

        if recievers[*n].contains(&second.transmitter_id)
        || recievers[*m].contains(&first.transmitter_id) {
            return false;
        }
    }

    return true;
}

fn transmission_recievers(analysis: &CompleteAnalysis) -> Vec<Vec<usize>> {
    analysis.transmissions.iter().map(|x| {
        analysis.sim_events.iter().filter_map(|event| {
            match event.content {
                LogContent::TransmissionReceived { receiver_id, transmission_id } => (transmission_id == x.id).then_some(receiver_id),
                _ => None,
            }
        }).collect()
    }).collect()
}

fn overlapping_transmissions(analysis: &CompleteAnalysis) -> Vec<(usize, usize)> {
    let mut output = Vec::new();

    for (n, first) in analysis.transmissions.iter().enumerate() {
        for (m, second) in analysis.transmissions.iter().enumerate() {
            if n < m && first.overlaps(second) {
                output.push((n, m));
            }
        }
    }

    output
}
