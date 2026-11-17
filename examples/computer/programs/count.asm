; Counts up on OUT: 0, 1, 2 and so on. After 255, it starts again at 0.
        LDI 0
loop:   OUT
        ADDI 1      ; A = A + 1
        JMP loop
