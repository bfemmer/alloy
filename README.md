

# **The Alloy Programming Language Manual**

### ***Official Language Specification and Reference***

## **1\. Introduction & Philosophy**

Alloy is a compiled systems programming language designed to occupy the unique educational gap between raw RISC-V Assembly and C.  
High-level programming languages hide the machine; raw assembly makes software development painfully tedious. Alloy balances these forces by maintaining a strict **1:1 relationship with the bare metal's register file** while abstracting away the boilerplate of control flow structures and address resolution.

### **Below C: Zero-Abstraction Registers**

Alloy does **not** feature a register allocator, variable scoping rules, or automated lifetime management. When you type let t0 \= 10, you are directly commanding the physical hardware register t0. If a function uses registers, it is up to the developer to preserve them on the stack pointer (sp).

### **Above Assembly: Structured Control Flow**

Alloy completely eliminates the need to write manual jump labels, conditional branch calculation steps, and memory alignment bookkeeping for strings and globals. You write clean, nested if, while, and for statements, alongside natural mathematical expressions (t0 \= t1 \+ t2), and the compiler cleanly lowers them into verified RISC-V assembly blocks.  
**Target Architecture:** RISC-V (64-bit Base Integer Architecture)  
**Compilation Target:** Fully-linked static native binaries (orchestrated via GCC toolchains)

## **2\. Compiler Architecture & Execution Pipeline**

The Alloy compiler operates as an orchestrated multi-stage driver, taking text source files and outputting high-performance native binaries.

### **2.1 Preprocessing & Module Imports**

When processing a source file, the compiler scans for import "filename" or import "filename.al" strings. The preprocessor handles imports using a three-tiered resolution hierarchy:

1. Searches relative to the location of the active file.  
2. Searches within a local lib/ directory relative to the source.  
3. Searches a global system library directory relative to the compiler executable itself.

All discovered modules are recursively flattened into a single unified source string, allowing simple multi-file project layouts.

### **2.2 Tokenization (Lexing)**

The stream of raw text characters is processed into discrete strings (tokens). The lexer breaks structures on whitespace boundaries and functional delimiters. Crucially, the lexer scans ahead to bind compound logical operators (\<=, \>=, \==, \!=) into single, coherent tokens, ensuring clean parsing phases later.

### **2.3 Parsing & AST Validation**

Alloy utilizes a Recursive Descent Parser to convert token sequences into a strongly-typed Abstract Syntax Tree (AST). During this phase, compile-time tracking tables evaluate definitions for constants, global memory layouts, and register aliasing before generating statements.

### **2.4 Code Generation & Hardware Constraint Protection**

The code generator transforms the verified AST directly into GNU-compatible RISC-V assembly code (.S). A major design feature of this stage is **Automatic Instruction Legalization**.  
Certain physical RISC-V extensions (such as the M-Extension for multiplication and division) explicitly forbid the use of raw numeric values (immediates) or global memory labels within operations (e.g., mul t0, t1, 10 is an invalid CPU instruction).  
Alloy continuously checks for these hardware limits, automatically generating code to stage problematic values inside hidden scratchpad registers (**t5** and **t6**) right before executing the calculation.

### **2.5 Assembly & Linkage**

The generated .S file is automatically fed into an internal system command runner using the toolchain binary specified in the local config.txt file (e.g., riscv64-unknown-linux-gnu-gcc or riscv64-unknown-elf-gcc). The compilation includes the \-static flag, ensuring all core libraries are embedded directly inside the binary.

## **3\. The Lexicon & Core Types**

### **3.1 Token Typings**

* **Comments:** Any text following a \# character up to the end of the line is ignored by the lexer.  
* **Keywords:** fn, ret, ecall, let, if, else, while, for, import, const, alias, data.  
* **Registers:** x0 through x31, alongside official ABI aliases (zero, ra, sp, gp, tp, a0-a7, t0-t6, s0-s11).  
* **Infix Operators:** \==, \!=, \<, \>, \<=, \>=, \+, \-, \*, /, %.  
* **Numeric Radix Literals:** \* *Decimal:* Standard format numbers (e.g., 42, \-12).  
  * *Hexadecimal:* Prefixed with 0x (e.g., 0xFF, 0x1000).  
* **Strings:** Wrapped in double quotes (e.g., "Result: \\n"). Stored as null-terminated character arrays (.asciz) automatically.

### **3.2 Symbol Tables: Compile-Time vs Runtime Storage**

Alloy manages code elements across three distinct structural types:

1. **Constants (const):** Pure immutable values mapped during compilation. They consume no physical RAM.  
2. **Aliases (alias):** Compile-time alternative labels pointing directly to hardware registers.  
3. **Globals (data):** Static variables initialized or reserved directly within the execution binary's persistent runtime memory layout.

## **4\. Syntax & Grammar Reference**

### **4.1 Top-Level Declarations**

#### **Constants (const)**

Constants are evaluated at compile time and substitute numbers directly into code instructions wherever they are called.

```alloy
const MAX_LIMIT = 100  
const BASE_ADDR = 0x4000

fn main() {  
    let t0 = MAX_LIMIT   # Lowers directly to: li t0, 100  
    let t1 = BASE_ADDR   # Lowers directly to: li t1, 16384  
    ret  
}
```

#### **Aliases (alias)**

Aliases swap human-readable names for hardware registers. They can be defined globally or locally at the top of code blocks.

```alloy
alias loop_index = t0  
alias output_ptr = a0

fn main() {  
    let loop_index = 0   # Moves 0 into register t0  
    let output_ptr = 1   # Moves 1 into register a0  
    ret  
}
```

#### **Global Data Allocations (data)**

The data keyword permanently assigns space inside the executable's .data memory block. Alloy supports primitive variables (Scalars) and continuous memory segments (Arrays).

```alloy
const BUFFER_SIZE = 64

data score = 0              # Single 32\-bit word initialized to 0  
data system_buffer\[64\]      # Reserves 64 bytes of sequential zeroed storage  
data large_array\[BUFFER_SIZE\] # Array sizing using compile-time constants

fn main() {  
    # Loading a global variable pointer into a register  
    let t0 = score          # la t0, score (loads memory address)  
    ret  
}
```

### **4.2 Assignments, Arithmetic, and Load/Store Operations**

Alloy offers two syntactic pathways for calculation instructions: **Infix Arithmetic** and **Functional Mnemonics**.

```alloy
fn main() {  
    # --- INFIX MATH EXPRESSIONS ---  
    let t0 = t1 + t2        # Lowered to: add t0, t1, t2  
    let t0 = t1 + 5         # Lowered to: addi t0, t1, 5  
    let t3 = t4 - 12        # Lowered to: addi t3, t4, -12  
    let t5 = t0 * t1        # Staged and multiplied via register flags

    # --- FUNCTIONAL MNEMONIC SYNTAX ---  
    let t0 = add(t1, t2)    # Identical to 't1 + t2'  
    let t1 = slt(t0, t5)    # Set Less Than instruction matching assembly  
      
    # --- MEMORY LOAD FORMAT   
    # To read from an address, use mnemonic formats: opcode(offset, base_register)  
    let t0 = lw(0, sp)      # Load Word from the top of the stack pointer  
    let t1 = lb(4, a0)      # Load Byte from an offset of 4 bytes from address a0  
      
    # --- MEMORY STORE FORMAT ---  
    # Storing to memory does not use 'let' because it does not update a register.  
    # Syntax: opcode(src_register, offset(base_register))  
    sw(t0, 0(sp))           # Store Word from t0 directly to the stack pointer offset 0  
    sb(t1, 8(s0))           # Store Byte from t1 to memory offset 8 from s0  
    ret  
}
```

### **4.3 Control Flow & Logical Operators**

Alloy supports high-level logical comparison notation inside all control flow blocks.  
**Hardware Note:** RISC-V hardware cannot natively handle immediate values on the left side of comparisons, nor does it possess native \> or \<= branch instructions. The Alloy compiler handles this automatically. If a comparison involves an immediate value (e.g., t0 \<= 10), the compiler passes a pseudo-opcode to the code generator, which automatically leverages the **t6** register to stage the immediate safely before emitting the branch.

#### **Conditionals (if / else)**

Conditional logic checks an explicit infix expression. If the comparison fails, execution jumps around or into the alternative code block.

```alloy
fn main() {  
    if (t0 == zero) {  
        let a0 = "Value is zero!\n"  
        call io_print  
    } else {  
        let a0 = "Value is non-zero!\n"  
        call io_print  
    }  
    ret  
}
```

#### **While Loops (while)**

while blocks re-evaluate a comparison rule at the top of every cycle, branching past the end label when the expression evaluates to false.

```alloy
fn main() {  
    let t0 = 0  
    while (t0 < 10) {  
        # Execute loop actions...  
        let t0 = t0 + 1  
    }  
    ret  
}
```

#### **For Loops (for)**

To match standard C-style loop familiarity, Alloy separates the initial statement, comparison expression, and incremental step statement using **semicolons (;)**:

```alloy
fn main() {  
    # for ( INIT_STATEMENT ; CONDITION_EXPRESSION ; STEP_STATEMENT )  
    for (let t0 = 0; t0 <= 10; let t0 = t0 + 1) {  
        let a0 = t0  
        call print_int  
    }  
    ret  
}
```

### **4.4 Input/Output and Standard Input (stdin)**

Because Alloy interfaces directly with the operating system kernel via RISC-V ecall mechanics, capturing user input from stdin is handled by invoking the Linux **System Read** syscall (sys\_read, Syscall \#63).

#### **The Input Mechanism (sys\_read)**

To read incoming data streams from standard input, the hardware register states must be configured as follows right before an ecall is triggered:

* a0 = 0 (File Descriptor: stdin)  
* a1 = **Buffer Address Pointer** (The starting memory location where characters will be stored)  
* a2 = **Max Length** (The maximum number of bytes the kernel is permitted to read)  
* a7 = 63 (The explicit Linux Syscall ID for sys_read)

#### **Continuous Memory Buffering on the Stack**

Since Alloy does not feature automated dynamic heap allocations, developers must manually reserve temporary buffer spaces by growing the Stack Pointer (sp) downwards.

```alloy
# --- STANDARD IO READ FUNCTION ---  
# Inputs: a1 = target buffer address, a2 = maximum bytes to read  
# Outputs: a0 = actual number of bytes read by the OS kernel  
fn io_read(a1, a2) {  
    let a0 = 0    # Set file descriptor to stdin  
    let a7 = 63   # Set syscall to sys_read  
    ecall  
    ret  
}

fn main() {  
    # 1. Manually carve out a 64\-byte character buffer space on the Stack Frame  
    let sp = sp - 64

    # 2. Print a console prompt using standard output handlers  
    let a0 = "Enter command sequence: "  
    call io_print

    # 3. Establish the arguments for the input stream read call  
    let a1 = sp + 0    # Pass the address pointing to the top of our stack buffer  
    let a2 = 63        # Reserve space for 63 text characters + 1 null terminator byte  
    call io_read       # Transfer execution control to the OS kernel reader

    # 4. Echo the received characters back to the user  
    let a0 = "Acknowledged: "  
    call io_print  
      
    let a0 = sp + 0    # Point to our manual stack buffer base address  
    call io_print      # Print the captured buffer back out to stdout

    # 5. Reclaim stack frame space  
    let sp = sp + 64  
    ret  
}
```

#### **ASCII to Integer Evaluation (atoi)**

When processing numeric parameters from standard input, the captured buffer contains raw text characters rather than evaluations ("123" $\\rightarrow$ ASCII bytes 0x31, 0x32, 0x33). To convert these elements into hardware-compatible integer states, a sequential mapping loop must parse the bytes:

```alloy
# --- ASCII TO INTEGER (atoi) ---  
# Input: a0 = pointer to string buffer  
# Output: a0 = parsed physical 32-bit integer value  
fn atoi(a0) {  
    let a1 = 0 # Initialize our running total accumulator to zero  
    call _atoi_processing_loop  
    ret  
}

fn _atoi_processing_loop(a0, a1) {  
    let t0 \= lb(0, a0) \# Load the active character byte

    # Halt iteration on a Null Terminator (0) or a Newline marker (10)  
    if (t0 == zero) { let a0 = a1 ret }  
    let t1 = 10  
    if (t0 == t1) { let a0 = a1 ret }

    # Character Validation: Confirm the active byte sits between '0' (48) and '9' (57)  
    let t1 = 48  
    if (t0 < t1) { let a0 = a1 ret }  
    let t1 = 58  
    if (t0 < t1) {  
        # Extract numeric value by subtracting the ASCII bias factor  
        let t0 = t0 - 48  
          
        # Accumulator Formula: total = (total * 10) + active_digit  
        let t2 = 10  
        let a1 = a1 * t2  
        let a1 = a1 + t0

        # Increment data address pointer and loop recursively  
        let a0 = a0 + 1  
        call _atoi_processing_loop  
        ret  
    } else {  
        let a0 = a1  
        ret  
    }  
}
```

## **5\. ABI Call Architectures & Core Memory Map**

Every function call naturally increments stack tracking depth. Because Alloy exposes machine access directly, users must closely follow the standard RISC-V Application Binary Interface (ABI) to safely isolate function contexts.

### **5.1 Function Calling Convention**

When entering any function via a call instruction, the CPU instantly overwrites the Return Address (ra) register. If your function calls another subroutine, **you will corrupt ra and crash the program unless you preserve it on the stack frame.**

```alloy
fn leaf_function() {  
    # This function calls nothing else (Leaf Node).  
    # It can safely use temporaries without saving 'ra'.  
    let t0 = 10 + 20  
    ret                     # Safe return using intact 'ra'  
}

fn nested_function() {  
    # This function calls other logic. We MUST allocate a stack frame!  
    let sp = sp - 16        # Grow stack down (aligned to 16 bytes)  
    sw(ra, 12(sp))          # Save original return address safely  
    sw(s0, 8(sp))           # Save frame base register if needed  
      
    call leaf_function      # 'ra' is updated with our local position  
      
    lw(s0, 8(sp))           # Restore tracking structures  
    lw(ra, 12(sp))          # Restore original destination address  
    let sp = sp + 16        # Reclaim stack space  
    ret                     # Jumps back to correct calling position  
}
```

### **5.2 System Register Reference Map**

| Register ID | ABI Designation | Architectural Role inside Alloy Codebases |
| :---- | :---- | :---- |
| x0 | zero | Hardwired Constant Zero. Writes are safely discarded. |
| x1 | ra | Return Address tracking. Updated by call, used by ret. |
| x2 | sp | Stack Pointer. Points to the current bottom of active memory. |
| x8 | s0 / fp | Frame pointer or Saved Register storage. |
| x10 - x17 | a0 - a7 | Argument passing channels. a0 acts as the function return value. a7 stores target System Call IDs. |
| x5 - x7 | t0 - t2 | Volatile Temporaries. Free-use registers. |
| x28 - x29 | t3 - t4 | Volatile Temporaries. Free-use registers. |
| x30 | t5 | **Compiler-Reserved Scratchpad.** Used for global memory loads. |
| x31 | t6 | **Compiler-Reserved Scratchpad.** Used for structural loop/immediate comparisons. |
| x18 - x27 | s2 - s11 | Saved Non-Volatile Registers. Must be restored if mutated. |

