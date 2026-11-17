; Shows the Fibonacci numbers on OUT: 1, 1, 2, 3, 5 and so on. After 233, the next number does
; not fit in a byte, so the carry starts the series again.
PREV = 0x00         ; the RAM addresses of the numbers
CUR = 0x01
NEXT = 0x02
start:  LDI 0
        ST PREV
        LDI 1       ; A = the current number
loop:   OUT
        ST CUR
        ADD PREV    ; A = the next number
        JC start    ; the next number does not fit in 8 bits
        ST NEXT
        LD CUR
        ST PREV
        LD NEXT
        JMP loop
