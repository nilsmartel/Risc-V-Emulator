mod reg;


fn main() {
    println!("Hello, world!");
}

#[derive(Default, Debug)]
struct CPU {
    registers: [u32; 32],
    pc: u32,
}

impl CPU {
    fn write_reg(&mut self, index: impl Into<usize>, value: u32) {
        let index: usize = index.into();

        // the 0 register is always 0
        if index == 0 {
            return;
        }

        self.registers[index] = value;
    }
}
