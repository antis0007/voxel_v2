use super::{Kind, Node};

#[derive(Clone, Copy)]
pub struct Process {
    pub inputs: [(usize, u32); 3],
    pub output: (usize, u32),
    pub ticks: u32,
    pub power: u32,
}
pub fn process(kind: Kind, recipe: usize) -> Option<Process> {
    let (inputs, output, ticks, power) = match (kind, recipe) {
        (Kind::Washer, i @ 0..=2) => ([(i, 64), (0, 0), (0, 0)], (i + 9, 192), 24, 12),
        (Kind::Crusher, i @ 0..=2) => ([(i, 64), (0, 0), (0, 0)], (i + 9, 128), 16, 8),
        (Kind::Furnace, i @ 9..=11) => ([(i, 64), (0, 0), (0, 0)], (i - 3, 64), 12, 6),
        (Kind::AlloyKiln, 0) => ([(6, 192), (7, 64), (0, 0)], (14, 256), 24, 10),
        (Kind::AlloyKiln, 1) => ([(8, 128), (3, 64), (0, 0)], (15, 128), 32, 12),
        (Kind::Fabricator, 0) => ([(6, 128), (4, 64), (0, 0)], (16, 128), 20, 10),
        (Kind::Fabricator, 1) => ([(5, 64), (12, 64), (18, 64)], (17, 64), 28, 14),
        (Kind::Cultivator, 0) => ([(13, 64), (4, 16), (0, 0)], (13, 128), 32, 8),
        (Kind::Cultivator, 1) => ([(13, 128), (0, 0), (0, 0)], (4, 64), 32, 8),
        _ => return None,
    };
    Some(Process {
        inputs,
        output,
        ticks,
        power,
    })
}
impl Kind {
    pub fn processor(self) -> bool {
        matches!(
            self,
            Self::Washer
                | Self::Crusher
                | Self::Furnace
                | Self::AlloyKiln
                | Self::Fabricator
                | Self::Cultivator
        )
    }
    pub fn modes(self) -> &'static [&'static str] {
        match self {
            Self::AlloyKiln => &[
                "Bronze: 3 copper + 1 tin → 4 bronze",
                "Steel: 2 iron + 1 coal → 2 steel",
            ],
            Self::Fabricator => &[
                "Coils: 2 copper + 1 fibre → 2 coils",
                "Core: 1 crystal + 1 circuit + 1 essence → 1 arcane core",
            ],
            Self::Cultivator => &[
                "Biomass: 1 reusable culture + 0.25 fibre → 2 biomass",
                "Lumen: 2 biomass → 1 fibre",
            ],
            _ => &[],
        }
    }
}
impl Node {
    pub fn accepts(&self, i: usize) -> bool {
        if self.filter.is_some_and(|filter| filter != i) {
            return false;
        }
        if self.kind.modes().is_empty() {
            return self.kind.accepts(i);
        }
        process(self.kind, self.mode as usize)
            .is_some_and(|r| r.inputs.iter().any(|&(item, n)| n > 0 && item == i))
    }
    pub fn process(&self) -> Option<Process> {
        self.recipe.and_then(|r| process(self.kind, r))
    }
    pub fn choose_recipe(&self) -> Option<usize> {
        let (start, end) = match self.kind {
            Kind::Crusher | Kind::Washer => (0, 3),
            Kind::Furnace => (9, 12),
            _ => (self.mode as usize, self.mode as usize + 1),
        };
        (start..end).find(|&i| {
            process(self.kind, i).is_some_and(|r| r.inputs.iter().all(|&(j, n)| self.input[j] >= n))
        })
    }
}
