; Paints random colors on the screen of the GPU, as the noise of an old TV. Each plot moves to the
; next pixel. The random chip is on port 13.
loop:   LD 13       ; A = a random byte
        ST 11       ; plot the color A (the GPU uses the low 4 bits)
        JMP loop
