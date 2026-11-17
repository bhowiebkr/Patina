; Shows the Fibonacci numbers on OUT. At the carry, it shows 0 and starts again.
; LDI k: A = k. ADD k: A = A + RAM k. OUT: OUT = A. ST k: RAM k = A. LD k: A = RAM k.
; JMP k: jump. JC k: jump on carry. JZ k: jump on zero. k is 0 to 15. RAM has 4 words.
start:  LDI 0
        ST 2        ; the number before = 0
        LDI 1
        ST 1        ; the number = 1
loop:   OUT
        ADD 2       ; A = the next number
        JC done     ; stop when the next number is more than 255
        ST 3
        LD 1
        ST 2        ; the number before = the number
        LD 3
        ST 1        ; the number = the next number
        JMP loop
done:   LDI 0
        OUT
        JZ start    ; A is 0, so the jump always occurs
