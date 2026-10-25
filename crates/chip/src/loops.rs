//! Combinational loop detection: logic with no register cut that feeds its
//! own inputs would oscillate, so the compiler rejects it.
use crate::compile::{Diagnostic, err, resolve};
use crate::design::Design;
use crate::registry::{Dir, Registry};

/// Combinational modules whose outputs feed back into their own inputs.
pub(crate) fn cycles(design: &Design, registry: &Registry, d: &mut Vec<Diagnostic>) {
    let comb: Vec<u16> = design
        .parts
        .iter()
        .filter(|p| registry.module(&p.module).is_some_and(|s| s.combinational))
        .map(|p| p.id)
        .collect();
    let successors = |id: u16| -> Vec<u16> {
        design
            .nets
            .iter()
            .filter(|n| {
                resolve(design, registry, n).iter().any(|(p, _, q)| p.id == id && q.dir == Dir::Drive)
            })
            .flat_map(|n| resolve(design, registry, n).into_iter().filter(|(_, _, q)| q.dir == Dir::Sink).map(|(p, _, _)| p.id).collect::<Vec<_>>())
            .filter(|s| comb.contains(s))
            .collect()
    };
    // Iterative DFS with colours: 1 = on the stack, 2 = done.
    let mut colour = vec![0u8; comb.len()];
    for start in 0..comb.len() {
        if colour[start] != 0 {
            continue;
        }
        let mut stack = vec![(start, successors(comb[start]), 0usize)];
        colour[start] = 1;
        while let Some((node, next, k)) = stack.last_mut() {
            if *k < next.len() {
                let succ = next[*k];
                *k += 1;
                let Some(j) = comb.iter().position(|&c| c == succ) else { continue };
                match colour[j] {
                    0 => {
                        colour[j] = 1;
                        let s = successors(comb[j]);
                        stack.push((j, s, 0));
                    }
                    1 => {
                        err(d, format!("Combinational Loop: #{}", comb[j]), None);
                        return;
                    }
                    _ => {}
                }
            } else {
                colour[*node] = 2;
                stack.pop();
            }
        }
    }
}

