use std::env;
use std::fs;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::process;
use std::vec::IntoIter;
use std::fmt::Write as FmtWrite;
use std::collections::HashMap;

// ==========================================
// CONFIGURATION
// ==========================================

fn load_assembler_command() -> String {
    let config_path = Path::new("config.txt");
    let default_cmd = "riscv64-unknown-elf-gcc".to_string();

    if !config_path.exists() {
        return default_cmd; // Fallback if config doesn't exist
    }

    if let Ok(content) = fs::read_to_string(config_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() {
                continue; // Skip comments and empty lines
            }
            if trimmed.starts_with("ASSEMBLER_CMD=") {
                let parts: Vec<&str> = trimmed.split('=').collect();
                if parts.len() == 2 {
                    return parts[1].trim().to_string();
                }
            }
        }
    }

    default_cmd
}

// ==========================================
// 1. THE AST
// ==========================================

#[derive(Debug, Clone, PartialEq)]
pub enum Register {
    Zero, Ra, Sp, Gp, Tp,
    A0, A1, A2, A3, A4, A5, A6, A7,
    T0, T1, T2, T3, T4, T5, T6,
    S0, S1, S2, S3, S4, S5, S6, S7, S8, S9, S10, S11,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Reg(Register),
    Imm(i32),
    Str(String),
    Label(String), // <--- NEW: For referring to global names
}

#[derive(Debug, Clone, PartialEq)]
pub enum Global {
    Scalar { name: String, value: i32 },
    Array { name: String, size: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    AluCall { opcode: String, operands: Vec<Operand> },
    Load { opcode: String, offset: i32, base: Register },
    Simple(Operand),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Let { target: Register, value: Expression },
    Store { opcode: String, src: Register, offset: i32, base: Register },
    Return,
    Ecall,
    Call { func_name: String },
    If { 
        condition_op: String, 
        left: Register, 
        right: Operand, 
        then_block: Vec<Statement>,
        else_block: Option<Vec<Statement>> 
    },
    While { 
        condition_op: String, 
        left: Register, 
        right: Operand, 
        body: Vec<Statement> 
    },
    For {
        init: Box<Statement>,
        condition_op: String,
        cond_left: Register,
        cond_right: Operand,
        step: Box<Statement>,
        body: Vec<Statement>
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub globals: Vec<Global>, // <--- NEW FIELD
    pub functions: Vec<Function>,
}

// ==========================================
// 2. THE LEXER
// ==========================================

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current_token = String::new();
    let mut in_string = false;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if in_string {
            if c == '"' {
                in_string = false;
                current_token.push('"');
                tokens.push(current_token.clone());
                current_token.clear();
            } else {
                current_token.push(c);
            }
        } else {
            match c {
                ';' => {
                    while let Some(&next_c) = chars.peek() {
                        if next_c == '\n' { break; }
                        chars.next();
                    }
                },
                '"' => {
                    if !current_token.is_empty() {
                        tokens.push(current_token.clone());
                        current_token.clear();
                    }
                    in_string = true;
                    current_token.push('"');
                },
                // Added '[' and ']' for array syntax
                '(' | ')' | '{' | '}' | '[' | ']' | ',' | '=' | '+' | '-' | '*' | '/' | '%' => {
                    if !current_token.is_empty() {
                        tokens.push(current_token.clone());
                        current_token.clear();
                    }
                    tokens.push(c.to_string());
                },
                c if c.is_whitespace() => {
                    if !current_token.is_empty() {
                        tokens.push(current_token.clone());
                        current_token.clear();
                    }
                },
                _ => current_token.push(c),
            }
        }
    }
    if !current_token.is_empty() { tokens.push(current_token); }
    tokens
}

// ==========================================
// 3. THE PARSER
// ==========================================

struct Parser {
    tokens: Peekable<IntoIter<String>>,
    constants: HashMap<String, i32>, 
    aliases: HashMap<String, Register>,
}

impl Parser {
    fn new(tokens: Vec<String>) -> Self {
        Self { 
            tokens: tokens.into_iter().peekable(),
            constants: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    fn peek(&mut self) -> Option<&String> { self.tokens.peek() }

    fn consume(&mut self, expected: &str) -> Result<(), String> {
        match self.tokens.next() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(format!("Expected '{}', found '{}'", expected, t)),
            None => Err(format!("Expected '{}', found EOF", expected)),
        }
    }

    fn parse_register(&mut self, token: &str) -> Result<Register, String> {
        // 1. Check Aliases
        if let Some(reg) = self.aliases.get(token) {
            return Ok(reg.clone());
        }

        // 2. Standard Registers
        match token {
            "x0" | "zero" => Ok(Register::Zero),
            "ra" => Ok(Register::Ra), "sp" => Ok(Register::Sp),
            "gp" => Ok(Register::Gp), "tp" => Ok(Register::Tp),
            "a0" => Ok(Register::A0), "a1" => Ok(Register::A1), "a2" => Ok(Register::A2),
            "a3" => Ok(Register::A3), "a4" => Ok(Register::A4), "a5" => Ok(Register::A5),
            "a6" => Ok(Register::A6), "a7" => Ok(Register::A7),
            "t0" => Ok(Register::T0), "t1" => Ok(Register::T1), "t2" => Ok(Register::T2),
            "t3" => Ok(Register::T3), "t4" => Ok(Register::T4), "t5" => Ok(Register::T5),
            "t6" => Ok(Register::T6),
            "s0" | "fp" => Ok(Register::S0), "s1" => Ok(Register::S1), "s2" => Ok(Register::S2),
            "s3" => Ok(Register::S3), "s4" => Ok(Register::S4), "s5" => Ok(Register::S5),
            "s6" => Ok(Register::S6), "s7" => Ok(Register::S7), "s8" => Ok(Register::S8),
            "s9" => Ok(Register::S9), "s10" => Ok(Register::S10), "s11" => Ok(Register::S11),
            _ => Err(format!("Unknown register: {}", token)),
        }
    }

    fn parse_operand(&mut self) -> Result<Operand, String> {
        let token = self.tokens.next().ok_or("Unexpected EOF")?;
        
        // Check Constants
        if let Some(&val) = self.constants.get(&token) {
            return Ok(Operand::Imm(val));
        }

        // Check Negative Numbers
        if token == "-" {
            let val_token = self.tokens.next().ok_or("Expected number after '-'")?;
            if let Some(&val) = self.constants.get(&val_token) {
                 return Ok(Operand::Imm(-val));
            }
            let val = val_token.parse::<i32>()
                .map_err(|_| format!("Invalid immediate value after '-': {}", val_token))?;
            return Ok(Operand::Imm(-val));
        }

        if token.starts_with('"') {
            return Ok(Operand::Str(token.trim_matches('"').to_string()));
        }
        if let Ok(reg) = self.parse_register(&token) { return Ok(Operand::Reg(reg)); }
        if token.starts_with("0x") {
             if let Ok(val) = i32::from_str_radix(token.trim_start_matches("0x"), 16) { return Ok(Operand::Imm(val)); }
        }
        if let Ok(val) = token.parse::<i32>() { return Ok(Operand::Imm(val)); }
        
        // If it's none of the above, it's a Label (Global Variable)
        Ok(Operand::Label(token))
    }

    fn parse_const(&mut self) -> Result<(), String> {
        self.consume("const")?;
        let name = self.tokens.next().ok_or("Expected constant name")?;
        self.consume("=")?;
        let val_token = self.tokens.next().ok_or("Expected value")?;
        
        let value = if val_token.starts_with("0x") {
            i32::from_str_radix(val_token.trim_start_matches("0x"), 16)
                .map_err(|_| format!("Invalid hex constant: {}", val_token))?
        } else if let Some(&existing_val) = self.constants.get(&val_token) {
            existing_val
        } else {
            val_token.parse::<i32>()
                .map_err(|_| format!("Invalid integer constant: {}", val_token))?
        };

        self.constants.insert(name, value);
        Ok(())
    }

    fn parse_alias(&mut self) -> Result<(), String> {
        self.consume("alias")?;
        let name = self.tokens.next().ok_or("Expected alias name")?;
        self.consume("=")?;
        let reg_token = self.tokens.next().ok_or("Expected register")?;
        let reg = self.parse_register(&reg_token)?;
        self.aliases.insert(name, reg);
        Ok(())
    }

    fn parse_data(&mut self) -> Result<Global, String> {
        self.consume("data")?;
        let name = self.tokens.next().ok_or("Expected global name")?;
        
        // Check for Array syntax: data buffer[100]
        if self.peek() == Some(&"[".to_string()) {
            self.consume("[")?;
            let size_token = self.tokens.next().ok_or("Expected array size")?;
            
            // Allow constants for size
            let size = if let Some(&val) = self.constants.get(&size_token) {
                val
            } else {
                size_token.parse::<i32>().map_err(|_| "Invalid size")?
            };
            
            self.consume("]")?;
            return Ok(Global::Array { name, size });
        }
        
        // Handle Scalar syntax: data score = 10
        self.consume("=")?;
        let val_token = self.tokens.next().ok_or("Expected value")?;
        let value = if let Some(&val) = self.constants.get(&val_token) {
            val
        } else {
            val_token.parse::<i32>().map_err(|_| "Invalid value")?
        };
        
        Ok(Global::Scalar { name, value })
    }

    fn parse_program(&mut self) -> Result<Program, String> {
        let mut functions = Vec::new();
        let mut globals = Vec::new();
        while let Some(token) = self.peek() {
            match token.as_str() {
                "fn" => { functions.push(self.parse_function()?); },
                "const" => { self.parse_const()?; },
                "alias" => { self.parse_alias()?; },
                "data" => { globals.push(self.parse_data()?); },
                _ => return Err(format!("Unexpected token at top level: {}", token)),
            }
        }
        Ok(Program { functions, globals })
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        self.consume("fn")?;
        let name = self.tokens.next().ok_or("Expected function name")?;
        self.consume("(")?;
        while let Some(t) = self.peek() { if t == ")" { break; } self.tokens.next(); }
        self.consume(")")?;
        self.consume("{")?;
        let body = self.parse_block()?;
        self.consume("}")?;
        Ok(Function { name, body })
    }

    fn parse_block(&mut self) -> Result<Vec<Statement>, String> {
        let mut statements = Vec::new();
        loop {
            let t = self.peek().ok_or("Unexpected EOF in block")?;
            if t == "}" || t == "else" { break; }
            
            if t == "alias" { self.parse_alias()?; continue; }
            if t == "const" { self.parse_const()?; continue; }

            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        let token = self.tokens.next().ok_or("Unexpected EOF")?;
        match token.as_str() {
            "ret" => Ok(Statement::Return),
            "ecall" => Ok(Statement::Ecall),
            "call" => {
                 let func_name = self.tokens.next().ok_or("Expected function name")?;
                 Ok(Statement::Call { func_name })
            },
            
            "for" => {
                self.consume("(")?;
                let init = Box::new(self.parse_statement()?);
                self.consume(",")?;
                
                let condition_op = self.tokens.next().ok_or("Expected condition op")?;
                let left_str = self.tokens.next().ok_or("Expected reg")?;
                let cond_left = self.parse_register(&left_str)?;
                if self.peek() == Some(&",".to_string()) { self.consume(",")?; }
                let cond_right = self.parse_operand()?;
                
                self.consume(",")?;
                let step = Box::new(self.parse_statement()?);
                self.consume(")")?;
                
                self.consume("{")?;
                let body = self.parse_block()?;
                self.consume("}")?;
                Ok(Statement::For { init, condition_op, cond_left, cond_right, step, body })
            },

            "while" => {
                self.consume("(")?;
                let condition_op = self.tokens.next().ok_or("Expected op")?;
                let left_str = self.tokens.next().ok_or("Expected reg")?;
                let left = self.parse_register(&left_str)?;
                if self.peek() == Some(&",".to_string()) { self.consume(",")?; }
                let right = self.parse_operand()?;
                self.consume(")")?;
                self.consume("{")?;
                let body = self.parse_block()?;
                self.consume("}")?;
                Ok(Statement::While { condition_op, left, right, body })
            },

            "if" => {
                self.consume("(")?;
                let condition_op = self.tokens.next().ok_or("Expected op")?;
                let left_str = self.tokens.next().ok_or("Expected reg")?;
                let left = self.parse_register(&left_str)?;
                if self.peek() == Some(&",".to_string()) { self.consume(",")?; } 
                let right = self.parse_operand()?;
                self.consume(")")?;
                self.consume("{")?;
                let then_block = self.parse_block()?;
                self.consume("}")?;
                let mut else_block = None;
                if let Some(t) = self.peek() {
                    if t == "else" {
                        self.consume("else")?;
                        self.consume("{")?;
                        else_block = Some(self.parse_block()?);
                        self.consume("}")?;
                    }
                }
                Ok(Statement::If { condition_op, left, right, then_block, else_block })
            },
            
            "sw" | "sb" | "sh" | "sd" => {
                let opcode = token; 
                self.consume("(")?;
                let src_token = self.tokens.next().unwrap();
                let src = self.parse_register(&src_token)?;
                
                if self.peek() == Some(&",".to_string()) {
                    self.consume(",")?;
                }

                let offset_token = self.tokens.next().unwrap();
                let offset = if let Some(&val) = self.constants.get(&offset_token) {
                    val
                } else {
                    offset_token.parse::<i32>().map_err(|_| format!("Invalid offset: {}", offset_token))?
                };

                self.consume("(")?;
                let base_token = self.tokens.next().unwrap();
                let base = self.parse_register(&base_token)?;
                self.consume(")")?; self.consume(")")?;
                Ok(Statement::Store { opcode, src, offset, base })
            },
            
            "let" => {
                let target_str = self.tokens.next().ok_or("Expected target")?;
                let target = self.parse_register(&target_str)?;
                self.consume("=")?;
                
                let next = self.peek().ok_or("Unexpected EOF")?.clone();
                
                // 1. Is it a "Standard" Operand? (Number, String, Register, Constant, Minus)
                let is_standard_op = next.starts_with('"') || 
                                     next.starts_with("0x") || 
                                     next.parse::<i32>().is_ok() || 
                                     next == "-" || 
                                     self.parse_register(&next).is_ok() || 
                                     self.constants.contains_key(&next);

                if is_standard_op {
                    // --- PATH A: Standard Math (e.g. let t0 = 5, let t0 = t1 + t2) ---
                    let left_op = self.parse_operand()?;
                    Self::parse_let_math_rest(self, target, left_op)

                } else {
                    // It is an Identifier. It could be a Mnemonic (add) OR a Label (score).
                    // We must consume it to check the NEXT token.
                    let name = self.tokens.next().unwrap();
                    
                    if self.peek() == Some(&"(".to_string()) {
                        // --- PATH B: Mnemonic (e.g. let t0 = add(t1, t2)) ---
                        self.consume("(")?;
                        if name.starts_with('l') && name != "lui" && name != "li" {
                            // Load Format: lw(offset, base)
                            let offset_token = self.tokens.next().unwrap();
                            let offset = if let Some(&val) = self.constants.get(&offset_token) {
                                val
                            } else {
                                offset_token.parse::<i32>().map_err(|_| "Invalid offset")?
                            };
                            self.consume("(")?;
                            let base_token = self.tokens.next().unwrap();
                            let base = self.parse_register(&base_token)?;
                            self.consume(")")?; self.consume(")")?;
                            Ok(Statement::Let { target, value: Expression::Load { opcode: name, offset, base } })
                        } else {
                            // ALU Format: add(a, b)
                            let mut operands = Vec::new();
                            while let Some(t) = self.peek() { 
                                if t == ")" { break; } 
                                if t == "," { self.tokens.next(); continue; }
                                operands.push(self.parse_operand()?); 
                            }
                            self.consume(")")?;
                            Ok(Statement::Let { target, value: Expression::AluCall { opcode: name, operands } })
                        }
                    } else {
                        // --- PATH C: Label Variable (e.g. let t0 = score) ---
                        // 'name' is the Label. Treat it as the Left Operand.
                        let left_op = Operand::Label(name);
                        Self::parse_let_math_rest(self, target, left_op)
                    }
                }
            },
            _ => Err(format!("Unknown token: {}", token)),
        }
    }

    // Helper to handle the rest of a math expression: "OP" or "OP + OP"
    fn parse_let_math_rest(&mut self, target: Register, left_op: Operand) -> Result<Statement, String> {
        let mut op_str_opt = None;
        if let Some(token) = self.peek() {
            op_str_opt = match token.as_str() {
                "+" => Some("add"), 
                "-" => Some("sub"), 
                "*" => Some("mul"), 
                "/" => Some("div"), 
                "%" => Some("rem"), 
                _ => None,
            };
        }

        if let Some(op_str) = op_str_opt {
            self.tokens.next(); // consume operator
            let right_op = self.parse_operand()?;
            
            let mut final_opcode = op_str.to_string();
            let mut final_operands = vec![left_op.clone(), right_op.clone()];

            if let Operand::Imm(val) = right_op {
                match op_str {
                    "add" => final_opcode = "addi".to_string(),
                    "sub" => {
                        final_opcode = "addi".to_string();
                        final_operands = vec![left_op, Operand::Imm(-val)];
                    },
                    _ => {} 
                }
            }
            Ok(Statement::Let { target, value: Expression::AluCall { opcode: final_opcode, operands: final_operands } })
        } else {
            Ok(Statement::Let { target, value: Expression::Simple(left_op) })
        }
    }
}

// ==========================================
// 4. THE CODE GENERATOR
// ==========================================

struct Codegen {
    text_buffer: String,
    string_table: Vec<(String, String)>,
    label_counter: usize,
}

impl Codegen {
    fn new() -> Self {
        Self { text_buffer: String::new(), string_table: Vec::new(), label_counter: 0 }
    }

    fn reg_name(r: &Register) -> &'static str {
        match r {
            Register::Zero => "zero", Register::Ra => "ra", Register::Sp => "sp",
            Register::Gp => "gp", Register::Tp => "tp",
            Register::A0 => "a0", Register::A1 => "a1", Register::A2 => "a2",
            Register::A3 => "a3", Register::A4 => "a4", Register::A5 => "a5",
            Register::A6 => "a6", Register::A7 => "a7",
            Register::T0 => "t0", Register::T1 => "t1", Register::T2 => "t2",
            Register::T3 => "t3", Register::T4 => "t4", Register::T5 => "t5",
            Register::T6 => "t6", Register::S0 => "s0", Register::S1 => "s1",
            Register::S2 => "s2", Register::S3 => "s3", Register::S4 => "s4",
            Register::S5 => "s5", Register::S6 => "s6", Register::S7 => "s7",
            Register::S8 => "s8", Register::S9 => "s9", Register::S10 => "s10", Register::S11 => "s11",
        }
    }

    fn emit_program(&mut self, prog: &Program) -> String {
        let mut final_output = String::new();
        writeln!(final_output, "# Generated by Alloy Compiler").unwrap();
        
        writeln!(final_output, ".data").unwrap();
        
        // 1. Emit Globals
        for global in &prog.globals {
            match global {
                Global::Scalar { name, value } => {
                    writeln!(final_output, "{}: .word {}", name, value).unwrap();
                },
                Global::Array { name, size } => {
                    writeln!(final_output, "{}: .zero {}", name, size).unwrap();
                }
            }
        }

        // 2. Emit Functions
        for func in &prog.functions { self.emit_function(func); }

        // 3. Emit String Literals
        if !self.string_table.is_empty() {
            for (label, content) in &self.string_table {
                writeln!(final_output, "{}:", label).unwrap();
                writeln!(final_output, "    .asciz \"{}\"", content).unwrap();
            }
        }
        
        writeln!(final_output, ".text").unwrap();
        final_output.push_str(&self.text_buffer);
        final_output
    }

    fn emit_function(&mut self, func: &Function) {
        let name = &func.name;
        writeln!(self.text_buffer, "\n.global {}", name).unwrap();
        writeln!(self.text_buffer, "{}:", name).unwrap();
        writeln!(self.text_buffer, "    addi sp, sp, -16").unwrap();
        writeln!(self.text_buffer, "    sw ra, 12(sp)").unwrap();
        writeln!(self.text_buffer, "    sw s0, 8(sp)").unwrap();
        writeln!(self.text_buffer, "    addi s0, sp, 16").unwrap();

        for stmt in &func.body { self.emit_statement(stmt, name); }

        writeln!(self.text_buffer, ".L_exit_{}:", name).unwrap();
        writeln!(self.text_buffer, "    lw s0, 8(sp)").unwrap();
        writeln!(self.text_buffer, "    lw ra, 12(sp)").unwrap();
        writeln!(self.text_buffer, "    addi sp, sp, 16").unwrap();
        writeln!(self.text_buffer, "    ret").unwrap();
    }
    
    fn prepare_comparison_operand(&mut self, op: &Operand) -> String {
        match op {
            Operand::Reg(r) => Self::reg_name(r).to_string(),
            Operand::Imm(i) => {
                writeln!(self.text_buffer, "    li t6, {}", i).unwrap();
                "t6".to_string()
            },
            Operand::Label(l) => {
                writeln!(self.text_buffer, "    lw t6, {}", l).unwrap();
                "t6".to_string()
            }
            _ => "zero".to_string(),
        }
    }

    fn emit_statement(&mut self, stmt: &Statement, func_name: &str) {
        match stmt {
            Statement::Return => { writeln!(self.text_buffer, "    j .L_exit_{}", func_name).unwrap(); },
            Statement::Ecall => { writeln!(self.text_buffer, "    ecall").unwrap(); },
            Statement::Call { func_name } => { writeln!(self.text_buffer, "    call {}", func_name).unwrap(); },
            Statement::Store { opcode, src, offset, base } => {
                writeln!(self.text_buffer, "    {} {}, {}({})", opcode, Self::reg_name(src), offset, Self::reg_name(base)).unwrap();
            },
            
            Statement::For { init, condition_op, cond_left, cond_right, step, body } => {
                let label_id = self.label_counter;
                self.label_counter += 1;
                let start_label = format!(".L_for_start_{}", label_id);
                let end_label = format!(".L_for_end_{}", label_id);

                self.emit_statement(init, func_name);
                writeln!(self.text_buffer, "{}:", start_label).unwrap();

                let branch_opcode = match condition_op.as_str() {
                    "beq" => "bne", "bne" => "beq", "blt" => "bge", "bge" => "blt", "slt" | "slti" => "bge", _ => "bne",
                };
                let right_op_str = self.prepare_comparison_operand(cond_right);
                writeln!(self.text_buffer, "    {} {}, {}, {}", branch_opcode, Self::reg_name(cond_left), right_op_str, end_label).unwrap();

                for s in body { self.emit_statement(s, func_name); }
                self.emit_statement(step, func_name);
                writeln!(self.text_buffer, "    j {}", start_label).unwrap();
                writeln!(self.text_buffer, "{}:", end_label).unwrap();
            },

            Statement::While { condition_op, left, right, body } => {
                let label_id = self.label_counter;
                self.label_counter += 1;
                let start_label = format!(".L_while_start_{}", label_id);
                let end_label = format!(".L_while_end_{}", label_id);

                writeln!(self.text_buffer, "{}:", start_label).unwrap();
                let branch_opcode = match condition_op.as_str() {
                    "beq" => "bne", "bne" => "beq", "blt" => "bge", "bge" => "blt", "slt" => "bge", _ => "bne",
                };
                let right_op_str = self.prepare_comparison_operand(right);
                writeln!(self.text_buffer, "    {} {}, {}, {}", branch_opcode, Self::reg_name(left), right_op_str, end_label).unwrap();
                for s in body { self.emit_statement(s, func_name); }
                writeln!(self.text_buffer, "    j {}", start_label).unwrap();
                writeln!(self.text_buffer, "{}:", end_label).unwrap();
            },

            Statement::If { condition_op, left, right, then_block, else_block } => {
                let label_id = self.label_counter;
                self.label_counter += 1;
                let else_label = format!(".L_else_{}", label_id);
                let end_label = format!(".L_end_{}", label_id);

                let jump_target = if else_block.is_some() { &else_label } else { &end_label };
                let branch_opcode = match condition_op.as_str() {
                    "beq" => "bne", "bne" => "beq", "blt" => "bge", "bge" => "blt", "slt" => "bge", _ => "bne",
                };
                let right_op_str = self.prepare_comparison_operand(right);
                writeln!(self.text_buffer, "    {} {}, {}, {}", branch_opcode, Self::reg_name(left), right_op_str, jump_target).unwrap();
                for s in then_block { self.emit_statement(s, func_name); }
                if let Some(e_block) = else_block {
                    writeln!(self.text_buffer, "    j {}", end_label).unwrap();
                    writeln!(self.text_buffer, "{}:", else_label).unwrap();
                    for s in e_block { self.emit_statement(s, func_name); }
                }
                writeln!(self.text_buffer, "{}:", end_label).unwrap();
            },

            Statement::Let { target, value } => {
                let dest = Self::reg_name(target);
                match value {
                    Expression::Simple(op) => {
                        match op {
                            Operand::Imm(i) => writeln!(self.text_buffer, "    li {}, {}", dest, i).unwrap(),
                            Operand::Reg(r) => writeln!(self.text_buffer, "    mv {}, {}", dest, Self::reg_name(r)).unwrap(),
                            Operand::Str(s) => {
                                let label = format!(".L_str_{}", self.label_counter);
                                self.label_counter += 1;
                                self.string_table.push((label.clone(), s.clone()));
                                writeln!(self.text_buffer, "    la {}, {}", dest, label).unwrap();
                            },
                            Operand::Label(l) => {
                                writeln!(self.text_buffer, "    la {}, {}", dest, l).unwrap();
                            }
                        }
                    },
                    Expression::AluCall { opcode, operands } => {
                        let needs_registers = ["mul", "div", "rem", "remu", "divu", "sub"].contains(&opcode.as_str());
                        let mut op_strs = Vec::new();
                        for (i, op) in operands.iter().enumerate() {
                            match op {
                                Operand::Reg(r) => op_strs.push(Self::reg_name(r).to_string()),
                                Operand::Imm(val) => {
                                    if needs_registers {
                                        let scratch = if i == 0 { "t5" } else { "t6" };
                                        writeln!(self.text_buffer, "    li {}, {}", scratch, val).unwrap();
                                        op_strs.push(scratch.to_string());
                                    } else {
                                        op_strs.push(val.to_string());
                                    }
                                },
                                Operand::Label(l) => {
                                    let scratch = if i == 0 { "t5" } else { "t6" };
                                    writeln!(self.text_buffer, "    lw {}, {}", scratch, l).unwrap();
                                    op_strs.push(scratch.to_string());
                                },
                                _ => panic!("Invalid operand"),
                            }
                        }
                        writeln!(self.text_buffer, "    {} {}, {}", opcode, dest, op_strs.join(", ")).unwrap();
                    },
                    Expression::Load { opcode, offset, base } => {
                         writeln!(self.text_buffer, "    {} {}, {}({})", opcode, dest, offset, Self::reg_name(base)).unwrap();
                    }
                }
            }
        }
    }
}

// ==========================================
// 5. MAIN (PREPROCESSOR + DRIVER)
// ==========================================

fn resolve_import_path(base_dir: &Path, import_name: &str) -> Option<PathBuf> {
    // 1. Check relative to current file
    let relative = base_dir.join(import_name);
    if relative.exists() { return Some(relative); }

    let relative_ext = base_dir.join(format!("{}.al", import_name));
    if relative_ext.exists() { return Some(relative_ext); }

    // 2. Check local "lib" folder relative to current file
    let local_lib = base_dir.join("lib").join(import_name);
    if local_lib.exists() { return Some(local_lib); }

    let local_lib_ext = base_dir.join("lib").join(format!("{}.al", import_name));
    if local_lib_ext.exists() { return Some(local_lib_ext); }

    // 3. Check "Standard Library" (relative to executable)
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let lib_path = exe_dir.join("lib").join(import_name);
            if lib_path.exists() { return Some(lib_path); }
            
            let lib_path_ext = exe_dir.join("lib").join(format!("{}.al", import_name));
            if lib_path_ext.exists() { return Some(lib_path_ext); }
        }
    }

    None
}

fn preprocess(filename: &Path) -> Result<String, String> {
    let source = fs::read_to_string(filename)
        .map_err(|e| format!("Error reading {}: {}", filename.display(), e))?;
    
    let mut final_code = String::new();
    let base_dir = filename.parent().unwrap_or(Path::new("."));

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import") {
            let parts: Vec<&str> = trimmed.split('"').collect();
            if parts.len() < 2 { return Err(format!("Invalid import: {}", line)); }
            
            let import_name = parts[1];
            
            let import_path = resolve_import_path(base_dir, import_name)
                .ok_or_else(|| format!("Import not found: {}", import_name))?;

            final_code.push_str(&preprocess(&import_path)?);
            final_code.push('\n');
        } else {
            final_code.push_str(line);
            final_code.push('\n');
        }
    }
    Ok(final_code)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { 
        eprintln!("Usage: {} <source.al>", args[0]); 
        process::exit(1); 
    }

    let input_path = Path::new(&args[1]);

    let assembler_cmd = load_assembler_command();

    // 1. Preprocess
    let source_code = match preprocess(input_path) {
        Ok(c) => c,
        Err(e) => { eprintln!("{}", e); process::exit(1); }
    };

    // 2. Tokenize
    let tokens = tokenize(&source_code);

    // 3. Parse
    let mut parser = Parser::new(tokens);
    let program = match parser.parse_program() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Parse Error: {}", e);
            process::exit(1);
        }
    };

    // 4. Generate Assembly
    let mut codegen = Codegen::new();
    let asm_output = codegen.emit_program(&program);
    
    // 5. Write .S file
    let asm_path = input_path.with_extension("S");
    if let Err(e) = fs::write(&asm_path, asm_output) {
        eprintln!("Error writing assembly file: {}", e);
        process::exit(1);
    }
    println!("Generated Assembly: {}", asm_path.display());

    // 6. Assemble & Link
    let exe_path = input_path.with_extension(""); 

    println!("Compiling to executable: {}", exe_path.display());

    let output = process::Command::new(&assembler_cmd)
        .arg(&asm_path)
        .arg("-o")
        .arg(&exe_path)
        .arg("-static")
        .output();

    match output {
        Ok(out) => {
            if out.status.success() {
                println!("✅ Success! Executable created: ./{}", exe_path.display());
            } else {
                eprintln!("❌ Assembler Error:");
                eprintln!("{}", String::from_utf8_lossy(&out.stderr));
                eprintln!("(Note: Ensure you have '{}' installed)", assembler_cmd);
            }
        },
        Err(e) => {
            eprintln!("❌ Failed to execute assembler: {}", e);
            eprintln!("(Check if '{}' is in your PATH)", assembler_cmd);
        }
    }
}