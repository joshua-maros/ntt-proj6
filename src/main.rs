use std::collections::HashMap;

#[derive(Clone, Copy)]
enum AssemblerState {
    LookingForInstruction,
    OneSlash,
    Comment,
}

enum PartiallyAssembledInstruction {
    Complete(u16),
    SymbolicAddress(String),
}

fn predefined_symbol_table() -> HashMap<String, u16> {
    let mut result = HashMap::new();
    for &(name, address) in &[
        ("SP", 0),
        ("LCL", 1),
        ("ARG", 2),
        ("THIS", 3),
        ("THAT", 4),
        ("SCREEN", 0x4000),
        ("KBD", 0x6000),
    ] {
        result.insert(String::from(name), address);
    }
    for r in 0..16 {
        result.insert(format!("R{}", r), r);
    }
    result
}

struct Assembler {
    state: AssemblerState,
    instruction_buffer: String,
    symbol_table: HashMap<String, u16>,
    extra_data_address: u16,
    output: Vec<PartiallyAssembledInstruction>,
}

impl Assembler {
    fn new() -> Self {
        Self {
            state: AssemblerState::LookingForInstruction,
            instruction_buffer: String::new(),
            symbol_table: predefined_symbol_table(),
            extra_data_address: 0b10000,
            output: Vec::new(),
        }
    }

    fn assemble_a_type_instruction(&mut self) {
        use PartiallyAssembledInstruction::*;
        let symbol_or_value = &self.instruction_buffer.trim()[1..];
        if let Ok(value) = symbol_or_value.parse::<usize>() {
            if value > 0b01111111_11111111 {
                panic!("The value {} is too big to use in an A instruction.", value);
            }
            self.output.push(Complete(value as u16));
        } else {
            self.output
                .push(SymbolicAddress(String::from(symbol_or_value)));
        }
    }

    fn assemble_c_type_instruction(&mut self) {
        let mut instruction = self.instruction_buffer.trim();
        let mut dest = 0;
        if let Some(index) = instruction.find("=") {
            let dest_name = &instruction[..index];
            if dest_name.contains("M") {
                dest |= 0b1;
            }
            if dest_name.contains("D") {
                dest |= 0b10;
            }
            if dest_name.contains("A") {
                dest |= 0b100;
            }
            instruction = &instruction[index + 1..];
        }
        let mut jmp = 0;
        if let Some(index) = instruction.find(";") {
            let jmp_name = &instruction[index + 1..];
            jmp = match jmp_name {
                "null" => 0b000,
                "JGT" => 0b001,
                "JEQ" => 0b010,
                "JGE" => 0b011,
                "JLT" => 0b100,
                "JNE" => 0b101,
                "JLE" => 0b110,
                "JMP" => 0b111,
                _ => panic!("{} is not a valid jump code", jmp_name),
            };
            instruction = &instruction[..index];
        }
        let comp = match instruction {
            "0" => 0b0_101010,
            "1" => 0b0_111111,
            "-1" => 0b0_111010,
            "D" => 0b0_001100,
            "A" => 0b0_110000,
            "!D" => 0b0_001101,
            "!A" => 0b0_110001,
            "-D" => 0b0_001111,
            "-A" => 0b0_110011,
            "D+1" => 0b0_011111,
            "A+1" => 0b0_110111,
            "D-1" => 0b0_001110,
            "A-1" => 0b0_110010,
            "D+A" => 0b0_000010,
            "D-A" => 0b0_010011,
            "A-D" => 0b0_000111,
            "D&A" => 0b0_000000,
            "D|A" => 0b0_010101,

            "M" => 0b1_110000,
            "!M" => 0b1_110001,
            "-M" => 0b1_110011,
            "M+1" => 0b1_110111,
            "M-1" => 0b1_110010,
            "D+M" => 0b1_000010,
            "D-M" => 0b1_010011,
            "M-D" => 0b1_000111,
            "D&M" => 0b1_000000,
            "D|M" => 0b1_010101,

            _ => panic!("{} is an invalid opcode", instruction),
        };
        let full = 0b111_00000_00000000 | comp << 6 | dest << 3 | jmp;
        self.output
            .push(PartiallyAssembledInstruction::Complete(full));
    }

    fn assemble_instruction(&mut self) {
        let trimmed = self.instruction_buffer.trim();
        if trimmed.len() > 0 {
            let first_char = trimmed.chars().next().unwrap();
            if first_char == '@' {
                // A-type instruction
                self.assemble_a_type_instruction()
            } else if first_char == '(' {
                // Label metainstruction
                let symbol_name = &trimmed[1..trimmed.len() - 1];
                self.symbol_table
                    .insert(String::from(symbol_name), self.output.len() as _);
            } else {
                self.assemble_c_type_instruction();
            }
        }
        self.instruction_buffer.clear();
    }

    fn assemble_source(mut self, source: &str) -> Vec<u16> {
        use AssemblerState::*;
        for c in source.chars() {
            match self.state {
                LookingForInstruction => match c {
                    '/' => self.state = OneSlash,
                    '\n' => self.assemble_instruction(),
                    _ => self.instruction_buffer.push(c),
                },
                OneSlash => match c {
                    '/' => self.state = Comment,
                    _ => {
                        self.state = LookingForInstruction;
                        self.instruction_buffer.push('/');
                        self.instruction_buffer.push(c);
                    }
                },
                Comment => match c {
                    '\n' => {
                        self.assemble_instruction();
                        self.state = LookingForInstruction;
                    }
                    _ => (),
                },
            }
        }
        self.assemble_instruction();
        self.finalize()
    }

    fn finalize_instruction(&mut self, instruction: PartiallyAssembledInstruction) -> u16 {
        use PartiallyAssembledInstruction::*;
        match instruction {
            Complete(value) => value,
            SymbolicAddress(symbol) => {
                if let Some(value) = self.symbol_table.get(&symbol) {
                    *value as _
                } else {
                    let address = self.extra_data_address;
                    self.extra_data_address += 1;
                    self.symbol_table.insert(symbol, address);
                    address
                }
            }
        }
    }

    fn finalize(mut self) -> Vec<u16> {
        let partial = std::mem::take(&mut self.output);
        partial
            .into_iter()
            .map(|i| self.finalize_instruction(i))
            .collect()
    }
}

fn assemble(source: &str) -> Vec<u16> {
    Assembler::new().assemble_source(source)
}

fn main() {
    let filename = std::env::args()
        .skip(1)
        .next()
        .expect("Must specify a filename.");
    let source = std::fs::read_to_string(&filename).expect("Failed to open file.");
    let instructions = assemble(&source[..]);
    let mut result = String::with_capacity(instructions.len() * 17);
    for instruction in instructions {
        result.push_str(&format!("{:016b}\n", instruction));
    }
    let output_name = filename.replace(".asm", ".hack");
    std::fs::write(&output_name, result).expect("Failed to write to output file.");
    println!("Wrote output to {}", output_name);
}
