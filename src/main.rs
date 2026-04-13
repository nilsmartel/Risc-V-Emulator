mod reg;

fn main() {
    println!("Hello, world!");
}

// RV64I
#[derive(Default, Debug)]
struct CPU {
    memory: Vec<u32>,
    registers: [u64; 32],
    pc: u32,
}

impl CPU {
    fn write_reg(&mut self, index: impl Into<usize>, value: u32) {
        let index: usize = index.into();

        // the 0 register is always 0
        if index == reg::ZERO {
            return;
        }

        self.registers[index] = value;
    }
}
