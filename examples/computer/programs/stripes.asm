; Paints the screen of the GPU with the 16 colors, one pixel after the other. Each plot moves to
; the next pixel, so the colors make stripes. Then the plot starts again at the top left.
        LDI 1
        ST 0        ; RAM 0 = 1
        LDI 0
loop:   ST 11       ; plot one pixel with the color A
        ADD 0       ; A = A + 1
        JMP loop
