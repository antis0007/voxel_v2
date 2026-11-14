//! Semi-Lagrangian transport and the pressure, Coriolis and drag update.

use glam::DVec3;

use super::forcing::KICK_PERIOD;
use super::ops::Operators;
use super::{DT, WAVE_SPEED_SQ, WeatherSim};

/// Linear drag on the wind, per time unit.
const DRAG: f64 = 0.06;
/// Rate at which pressure relaxes toward its equilibrium, per time unit.
const RELAX: f64 = 0.1;
/// Neighbour-mean blend per step that removes grid-scale noise.
const SMOOTH: f64 = 0.05;

/// Backtraced source of one cell: the nearest cell to the departure point and the offset to it.
pub(super) struct Source {
    pub cell: usize,
    pub offset: DVec3,
}

pub(super) struct Scratch {
    pub sources: Vec<Source>,
    pub gradient: Vec<DVec3>,
    pub scalar_in: Vec<f64>,
    pub scalar_out: Vec<f64>,
    pub wind: Vec<DVec3>,
    pub coriolis: Vec<(f64, f64)>,
}

/// Moves `field` along the traced sources with a limited linear reconstruction.
pub(super) fn transport(ops: &Operators, field: &[f64], sources: &[Source], grad: &mut [DVec3], out: &mut [f64]) {
    ops.gradient(field, grad);
    for (i, o) in out.iter_mut().enumerate() {
        let s = &sources[i];
        let value = field[s.cell] + grad[s.cell].dot(s.offset);
        let mut lo = field[s.cell];
        let mut hi = lo;
        for &j in &ops.nb[s.cell] {
            let f = field[j as usize];
            lo = lo.min(f);
            hi = hi.max(f);
        }
        *o = value.clamp(lo, hi);
    }
}

impl WeatherSim {
    pub(super) fn step_dynamics(&mut self) {
        let len = self.ops.len();
        let per_kick = (KICK_PERIOD / DT).round() as u64;
        if self.steps.is_multiple_of(per_kick) {
            self.kick(self.steps / per_kick);
        }
        let scratch = &mut self.scratch;
        scratch.sources.clear();
        for i in 0..len {
            let p = (self.ops.centre[i] - self.v[i] * DT).normalize();
            let cell = self.ops.locate(i, p);
            scratch.sources.push(Source { cell, offset: p - self.ops.centre[cell] });
        }
        for field in [&mut self.h, &mut self.temp, &mut self.humidity, &mut self.cloud] {
            transport(&self.ops, field, &scratch.sources, &mut scratch.gradient, &mut scratch.scalar_out);
            std::mem::swap(field, &mut scratch.scalar_out);
        }
        for k in 0..3 {
            for (dst, v) in scratch.scalar_in.iter_mut().zip(&self.v) {
                *dst = v[k];
            }
            transport(&self.ops, &scratch.scalar_in, &scratch.sources, &mut scratch.gradient, &mut scratch.scalar_out);
            for (v, &component) in scratch.wind.iter_mut().zip(&scratch.scalar_out) {
                v[k] = component;
            }
        }
        std::mem::swap(&mut self.v, &mut scratch.wind);

        self.ops.gradient(&self.h, &mut scratch.gradient);
        for i in 0..len {
            let n = self.ops.centre[i];
            let mut v = self.v[i] - scratch.gradient[i] * (WAVE_SPEED_SQ * DT);
            // dv/dt = -f n x v: rotate by -f dt about the local vertical.
            let (s, c) = scratch.coriolis[i];
            v = v * c + n.cross(v) * s;
            v *= 1.0 - DRAG * DT;
            self.v[i] = v - n * n.dot(v);
        }
        self.ops.divergence(&self.v, &mut self.div);
        for i in 0..len {
            let h = &mut self.h[i];
            *h += DT * (-self.div[i] + RELAX * (self.h_eq[i] - *h));
        }
        self.ops.smooth(&mut self.h, &mut scratch.scalar_out, SMOOTH);
        self.ops.smooth_vec(&mut self.v, &mut scratch.wind, SMOOTH);
    }
}
