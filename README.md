# **The Alloy Programming Language**

### ***Structured like C, Built like Assembly***

## **1\. Introduction & Philosophy**

Alloy is a compiled systems programming language designed to occupy the unique educational gap between raw RISC-V Assembly and C.

* **Below C:** Alloy does **not** manage memory or registers for you. There is no register allocator. If you write let t0 \= 10, you are manually occupying the hardware register t0. You must manually manage the stack pointer (sp) when calling functions.  
* **Above Assembly:** Alloy abstracts away the tedious flow control of assembly. Instead of manually creating jump labels (.L\_loop\_start) and calculating offsets, you use high-level structures like if, while, for, and mathematical expressions (t0 \+ t1).

**Target Architecture:** RISC-V (64-bit)  
**Output:** Native Executables (via GCC linkage)

## **2\. The Lexicon (Tokens)**

The Alloy lexer splits source code into a stream of tokens based on the following rules:

1. **Keywords:** fn, ret, ecall, let, if, else, while, for, import.  
2. **Registers:** Explicit RISC-V register names (x0..x31, zero, ra, sp, gp, tp, a0-a7, t0-t6, s0-s11).  
3. **Integers:**  
   * **Decimal:** 10, -5  
   * **Hexadecimal:** 0xFF  
4. **Strings:** Enclosed in double quotes "Hello World". The compiler automatically manages storage in the .data section.  
5. **Operators:** +, -, *, /, %, =, ,, (, ), {, }.  
6. **Comments:** Lines starting with ; are ignored.

## **3\. The Grammar (Syntax)**

### **3.1 Program Structure**

A program consists of one or more functions. The entry point is always main.

```alloy
import "lib/std_io.al"

fn main() {  
    ; Code goes here  
    ret  
}
```

### **3.2 Assignments & Arithmetic**

Assignments use the let keyword. Alloy supports both functional-style mnemonics (mapping directly to assembly instructions) and infix math operators.

```alloy
; Direct Value Load  
let t0 = 10           ; li t0, 10  
let t1 = 0xFF         ; li t1, 255

; Register Copy  
let t2 = t0           ; mv t2, t0

; Infix Math (Syntactic Sugar)  
let t0 = t1 + 5       ; addi t0, t1, 5  
let t1 = t2 - t3      ; sub t1, t2, t3  
let t4 = t0 * t1      ; mul t4, t0, t1

; Functional Mnemonic Style (Raw Assembly mapping)  
let t0 = add(t1, 5)   ; addi t0, t1, 5  
let t2 = slt(t0, t1)  ; Set Less Than
```

### **3.3 Memory Access**

Memory instructions (sw, lw, sb, lb, etc.) explicitly use the RISC-V offset syntax offset(base).

```alloy
; Store Word: Save t0 to stack at offset 0  
sw(t0, 0(sp))

; Load Word: Load from stack offset 4 into a0  
let a0 \= lw(4(sp))
```

### **3.4 Control Flow**

Alloy manages labels and branching logic automatically. Note that comparison instructions (like beq, slt) are used as the condition.  

**If / Else:**

```alloy
; Syntax: if ( COMPARISON ) { BODY }  
if (beq t0, t1) {  
    ; Runs if t0 \== t1  
} else {  
    ; Runs otherwise  
}
```

**While Loops:**

```alloy
; Syntax: while ( COMPARISON ) { BODY }  
while (slt t0, 10) {  
    ; Runs while t0 \< 10  
    let t0 \= t0 \+ 1  
}
```

**For Loops:**  
Uses comma delimiters instead of semicolons.

```alloy
; Syntax: for ( INIT , CONDITION , STEP ) { BODY }  
for ( let t0 \= 0 , slt t0, 10 , let t0 \= t0 \+ 1 ) {  
    ; Body code  
}
```

**Immediate Comparison Handling:**

* Assembly forbids comparing a register to a raw number (e.g., bge t0, 10).  
* **Compiler Magic:** Alloy automatically detects this. It generates code to load the immediate 10 into the reserved scratch register **t6**, and then compares t0 vs t6.

### **3.5 System Calls & Strings**

String literals are allocated in the .data section, and their address is loaded into the target register.

```alloy
; String Literal  
let a0 \= "Hello, World\!\\n"  ; Compiler emits .asciz and 'la a0, label'

; System Call  
let a0 \= 1     ; stdout  
let a7 \= 64    ; sys\_write  
ecall
```

## **4\. Abstract Syntax Tree (AST)**

The AST is the compiler's internal representation of your code. It is strictly typed using Rust enums.

```rust
pub enum Register { A0, A1, ... T0, T1, ... SP, RA, ... }

pub enum Operand {  
    Reg(Register),  
    Imm(i32),  
    Str(String),  
}

pub enum Expression {  
    // Represents: let t0 \= add(t1, t2) OR let t0 \= t1 \+ t2  
    AluCall { opcode: String, operands: Vec\<Operand\> },  
      
    // Represents: let t0 \= lw(0(sp))  
    Load { opcode: String, offset: i32, base: Register },  
      
    // Represents: let t0 \= 5  
    Simple(Operand),  
}

pub enum Statement {  
    // Assignment  
    Let { target: Register, value: Expression },  
      
    // Memory Store  
    Store { opcode: String, src: Register, offset: i32, base: Register },  
      
    // Flow Control  
    If { condition\_op: String, left: Register, right: Operand, then\_block: Vec\<Statement\>, else\_block: Option\<Vec\<Statement\>\> },  
    While { condition\_op: String, left: Register, right: Operand, body: Vec\<Statement\> },  
    For { init: Box\<Statement\>, condition\_op: String, cond\_left: Register, cond\_right: Operand, step: Box\<Statement\>, body: Vec\<Statement\> },  
      
    // Function Calls & Misc  
    Call { func\_name: String },  
    Return,  
    Ecall,  
}
```

## **5\. Compiler Architecture**

The Alloy compiler follows a classic 5-stage pipeline:

### **Phase 1: Preprocessing**

* **Input:** Source file (main.al).  
* **Action:** Scans for import "filename" directives. It recursively reads the target files and merges them into a single raw string of source code.

### **Phase 2: Tokenization (Lexing)**

* **Input:** Raw Source String.  
* **Action:** Converts text into a vector of strings. Handles splitting operands while preserving quoted strings and negative numbers.  
* *Example:* let t0 \= \-5 $\\rightarrow$ \["let", "t0", "=", "-5"\]

### **Phase 3: Parsing**

* **Input:** Token Vector.  
* **Action:** Recursive Descent Parser. Iterates through tokens to build the **AST**.  
* **Logic:**  
  * Detects fn to start a function.  
  * Detects let to parse assignments (handling both add() syntax and infix \+).  
  * Detects control structures (if, while), recursively parsing their bodies into blocks.

### **Phase 4: Code Generation**

* **Input:** AST.  
* **Action:** Walks the AST and emits RISC-V assembly text (.S).  
* **Key Responsibilities:**  
  * **String Table:** Collects all string literals encountered, assigning them labels (.L\_str\_0), and emitting them in the .data section.  
  * **Label Management:** Generates unique labels for loops (.L\_while\_start\_1) and conditionals (.L\_else\_2).  
  * **Operand Prep:** Detects immediate comparisons (e.g., t0 \< 10\) and injects instructions to load 10 into t6 before branching.

### **Phase 5: Assembly & Linking (Driver)**

* **Input:** Generated .S file.  
* **Action:** Invokes the external GCC toolchain (riscv64-linux-gnu-gcc).  
* **Output:** A fully linked executable binary.

## **6\. Runtime Model & Register Convention**

Since Alloy provides direct register access, users must adhere to the standard RISC-V calling convention to ensure their code interacts correctly with libraries and system calls.

| Register | Alloy Name | Role | Notes |
| :---- | :---- | :---- | :---- |
| x0 | zero | Hardwired Zero | Always 0\. Writing to it does nothing. |
| x1 | ra | Return Address | Saved automatically by call. **Must** save to stack if calling other functions. |
| x2 | sp | Stack Pointer | Manually managed. Grow down (subtract), shrink up (add). |
| x10-x17 | a0-a7 | Arguments / Return | a0 is return value. a7 is syscall ID. |
| x5-x7, x28-x31 | t0-t6 | Temporaries | t6 is used by the compiler for immediate comparisons. |
| x8, x9, x18-x27 | s0-s11 | Saved Registers | Preserved across calls. |

### **Compiler-Reserved Registers**

* **t6**: The Alloy Code Generator uses t6 as a scratchpad when you compare a register to a number (e.g., if (beq t0, 5)). **Avoid using t6 for long-term storage inside loops or conditionals.**
