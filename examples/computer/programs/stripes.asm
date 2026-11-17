; Paints the screen of the GPU with the 16 colors, one pixel after the other. Each plot moves to
; the next pixel, so the colors make stripes. Then the plot starts again at the top left.
PLOT = 0xF3         ; ST plots the color A, then the GPU X moves one pixel
        LDI 0
loop:   ST PLOT
        ADDI 1      ; the next color. The GPU uses the low 4 bits.
        JMP loop
