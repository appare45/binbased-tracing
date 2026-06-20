mod blob;
mod patch;
mod trampoline;

pub use patch::BranchStrategy;
pub use trampoline::build_trampoline;

#[derive(Clone)]
pub struct Instructions(Vec<u32>);

impl Instructions {
    pub fn new() -> Self {
        Instructions(Vec::new())
    }

    pub fn push(&mut self, inst: u32) {
        self.0.push(inst);
    }

    pub fn join(&mut self, value: Instructions) {
        value.0.iter().for_each(|v| self.push(*v));
    }

    pub fn get(&self, index: usize) -> Option<u32> {
        self.0.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Into<Vec<i64>> for Instructions {
    fn into(self) -> Vec<i64> {
        self.0
            .chunks_exact(2)
            .map(|chunk| {
                let low = chunk[0] as i64;
                let high = chunk[1] as i64;
                (high << 32) | (low & 0xFFFFFFFF)
            })
            .collect()
    }
}

impl From<Vec<i64>> for Instructions {
    fn from(value: Vec<i64>) -> Self {
        let mut instruction = Instructions::new();
        value.iter().for_each(|v| {
            let high = *v as u32;
            let low = (v >> 32) as u32;
            instruction.push(high);
            instruction.push(low);
        });
        instruction
    }
}

impl From<i64> for Instructions {
    fn from(value: i64) -> Self {
        let mut instruction = Instructions::new();
        let high = value as u32;
        let low = (value >> 32) as u32;
        instruction.push(high);
        instruction.push(low);
        instruction
    }
}
